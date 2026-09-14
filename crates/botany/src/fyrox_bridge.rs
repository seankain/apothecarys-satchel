//! `plantgl` geometry into Fyrox scene nodes.
//!
//! MIT, like the rest of `crates/botany`, and behind the `fyrox` feature so
//! the headless crates that depend on this one never pull the engine in.
//!
//! # What the conversion actually costs
//!
//! Almost nothing. A [`TriangleSet`] is already structure-of-arrays and
//! `plantgl`'s `real_t` is `f32`, which is Fyrox's vertex format too — so this
//! is a repack into the interleaved [`StaticVertex`] plus an index map, with
//! no numeric conversion anywhere.
//!
//! The one wrinkle is that a `TriangleSet` may carry *three* index lists —
//! positions, normals and texture coordinates, each addressed per face corner
//! — where a GPU vertex buffer has one. [`to_surface_data`] resolves that the
//! only way it can be resolved: a vertex per distinct corner triple, deduped.
//!
//! # Merge before you convert
//!
//! [`scene_to_node`] merges the scene by appearance *before* discretising, so
//! a plant arrives as one surface per material — four, in practice — rather
//! than one per leaf. Doing it the other way round would hand the renderer a
//! few hundred draw calls and then ask it to sort them out.

use std::collections::HashMap;

use fyrox::{
    asset::untyped::ResourceKind,
    core::{
        algebra::{Vector2, Vector3},
        color::Color,
        math::TriangleDefinition,
        pool::Handle,
    },
    material::{Material as FyroxMaterial, MaterialResource},
    scene::{
        base::BaseBuilder,
        graph::Graph,
        mesh::{
            buffer::{TriangleBuffer, VertexBuffer},
            surface::{SurfaceBuilder, SurfaceData, SurfaceResource},
            vertex::StaticVertex,
            MeshBuilder, RenderPath,
        },
        node::Node,
    },
};
use plantgl::algo::discretize::Discretizer;
use plantgl::algo::normals::{smooth_normals, DEFAULT_CREASE_ANGLE};
use plantgl::algo::{merge_scene, tessellate};
use plantgl::math::{Point3, Vec2, Vec3};
use plantgl::modelling::MeshBatch;
use plantgl::scenegraph::{Appearance, AppearanceRef, TriangleSet};
use plantgl::{DiscretizeCtx, Error, Result, Scene};

use crate::interpret::PlantModel;

/// One `plantgl` triangle set as a Fyrox surface.
///
/// Normals come from the mesh when it has them and are computed with a crease
/// angle when it does not — smooth along a sweep, hard at a cap, which is what
/// a discretised plant needs and what neither of PlantGL's own two modes give.
/// Texture coordinates default to the origin where a mesh carries none;
/// tangents are computed only when there are real coordinates to compute them
/// from, since a degenerate UV triangle has no tangent.
pub fn to_surface_data(mesh: &TriangleSet) -> Result<SurfaceData> {
    let flat = flatten(mesh)?;
    let has_tex_coords = mesh.model.has_tex_coords();
    let shaded = match flat.model.normals {
        Some(_) => flat,
        None => smooth_normals(&flat, DEFAULT_CREASE_ANGLE)?,
    };

    let points: &[Point3] = &shaded.model.points;
    let normals = shaded.model.normals.as_deref().ok_or_else(|| {
        Error::degenerate("a shaded mesh with no normals")
    })?;
    let tex_coords: Option<&[Vec2]> = shaded.model.tex_coords.as_deref().map(Vec::as_slice);

    let vertices: Vec<StaticVertex> = points
        .iter()
        .enumerate()
        .map(|(i, point)| StaticVertex {
            position: Vector3::new(point.x, point.y, point.z),
            tex_coord: tex_coords
                .and_then(|list| list.get(i))
                .map(|uv| Vector2::new(uv.x, uv.y))
                .unwrap_or_default(),
            normal: to_vector3(normals.get(i).copied().unwrap_or_else(Vec3::z)),
            tangent: Default::default(),
        })
        .collect();

    let triangles: Vec<TriangleDefinition> = shaded
        .indices
        .iter()
        .map(|face| TriangleDefinition([face[0], face[1], face[2]]))
        .collect();

    let buffer = VertexBuffer::new(vertices.len(), vertices)
        .map_err(|e| Error::codec(format!("vertex buffer: {e:?}")))?;
    let mut data = SurfaceData::new(buffer, TriangleBuffer::new(triangles));
    if has_tex_coords {
        data.calculate_tangents()
            .map_err(|e| Error::codec(format!("tangents: {e:?}")))?;
    }
    Ok(data)
}

/// A `plantgl` appearance as a Fyrox material.
///
/// PlantGL's diffuse colour is its ambient colour times a diffuse multiplier,
/// and its `transparency` is the complement of alpha; both are folded in here.
/// A shape with no appearance gets the standard material untouched.
pub fn to_material(appearance: Option<&Appearance>) -> MaterialResource {
    let mut material = FyroxMaterial::standard();
    if let Some(appearance) = appearance {
        let color = appearance.base_color();
        let alpha = match appearance {
            Appearance::Material(m) => {
                ((1.0 - m.transparency).clamp(0.0, 1.0) * 255.0).round() as u8
            }
            Appearance::Texture2D(_) => 255,
        };
        material.set_property(
            "diffuseColor",
            Color::from_rgba(color.red, color.green, color.blue, alpha),
        );
    }
    MaterialResource::new_ok(ResourceKind::Embedded, material)
}

/// A whole [`Scene`] as one mesh node, one surface per appearance.
///
/// `ctx` is the tessellation density — [`LodTier::discretize_ctx`] is where a
/// plant's comes from.
///
/// [`LodTier::discretize_ctx`]: crate::lod::LodTier::discretize_ctx
pub fn scene_to_node(scene: &Scene, graph: &mut Graph, ctx: DiscretizeCtx) -> Result<Handle<Node>> {
    let mut discretizer = Discretizer::new(ctx);
    let batches: Vec<MeshBatch> = merge_scene(scene, &mut discretizer)?
        .into_iter()
        .map(|batch| {
            Ok(MeshBatch {
                appearance: batch.appearance,
                mesh: tessellate(&batch.geometry)?,
            })
        })
        .collect::<Result<_>>()?;
    batches_to_node(&batches, graph, "Plant")
}

/// Already-merged batches as one mesh node — the path
/// [`plantgl::MeshDrawer`](plantgl::modelling::MeshDrawer) and
/// [`PlantModel::batches`] both produce, which skips the scene graph entirely.
pub fn batches_to_node(
    batches: &[MeshBatch],
    graph: &mut Graph,
    name: &str,
) -> Result<Handle<Node>> {
    let mut surfaces = Vec::with_capacity(batches.len());
    for batch in batches {
        if batch.mesh.face_count() == 0 {
            continue;
        }
        let data = to_surface_data(&batch.mesh)?;
        surfaces.push(
            SurfaceBuilder::new(SurfaceResource::new_ok(ResourceKind::Embedded, data))
                .with_material(to_material(batch.appearance.as_deref()))
                .build(),
        );
    }
    Ok(MeshBuilder::new(BaseBuilder::new().with_name(name))
        .with_surfaces(surfaces)
        .with_render_path(RenderPath::Forward)
        .build(graph))
}

/// A generated plant as a node, at the tier it was built for.
pub fn plant_to_node(model: &PlantModel, graph: &mut Graph) -> Result<Handle<Node>> {
    batches_to_node(&model.batches()?, graph, "Plant")
}

/// The appearances a set of batches uses, in batch order — what a caller that
/// wants to tint or swap materials afterwards needs.
pub fn batch_appearances(batches: &[MeshBatch]) -> Vec<Option<AppearanceRef>> {
    batches.iter().map(|b| b.appearance.clone()).collect()
}

fn to_vector3(v: Vec3) -> Vector3<f32> {
    Vector3::new(v.x, v.y, v.z)
}

/// Collapses a triangle set's three index lists into one.
///
/// A GPU vertex buffer addresses positions, normals and texture coordinates
/// with a single index, so a corner that reuses a position with a different
/// normal or a different UV has to become its own vertex. The map is keyed on
/// the corner's whole index triple, so nothing is duplicated that need not be.
fn flatten(mesh: &TriangleSet) -> Result<TriangleSet> {
    let source_points: &[Point3] = &mesh.model.points;
    let source_normals = mesh.model.normals.as_deref();
    let source_tex = mesh.model.tex_coords.as_deref();

    let mut points: Vec<Point3> = Vec::with_capacity(source_points.len());
    let mut normals: Vec<Vec3> = Vec::new();
    let mut tex_coords: Vec<Vec2> = Vec::new();
    let mut seen: HashMap<(u32, u32, u32), u32> = HashMap::new();
    let mut indices = Vec::with_capacity(mesh.face_count());

    for face in 0..mesh.face_count() {
        let mut corners = [0u32; 3];
        for (j, corner) in corners.iter_mut().enumerate() {
            let point = mesh
                .face_point_index_at(face, j)
                .ok_or_else(|| Error::invalid_index(format!("face {face} corner {j}")))?;
            let normal = source_normals
                .map(|_| mesh.face_normal_index_at(face, j).unwrap_or(point))
                .unwrap_or(point);
            let tex = source_tex
                .map(|_| mesh.face_tex_coord_index_at(face, j).unwrap_or(point))
                .unwrap_or(point);

            *corner = match seen.get(&(point, normal, tex)) {
                Some(index) => *index,
                None => {
                    let index = points.len() as u32;
                    points.push(*source_points.get(point as usize).ok_or_else(|| {
                        Error::invalid_index(format!("point {point} is past the end"))
                    })?);
                    if let Some(list) = source_normals {
                        normals.push(list.get(normal as usize).copied().unwrap_or_else(Vec3::z));
                    }
                    if let Some(list) = source_tex {
                        tex_coords.push(list.get(tex as usize).copied().unwrap_or_else(Vec2::zeros));
                    }
                    seen.insert((point, normal, tex), index);
                    index
                }
            };
        }
        indices.push(corners);
    }

    let mut flat = TriangleSet::new(points, indices);
    flat.model.ccw = mesh.model.ccw;
    flat.model.solid = mesh.model.solid;
    if source_normals.is_some() {
        flat.model.normals = Some(std::sync::Arc::new(normals));
        flat.model.normal_per_vertex = true;
    }
    if source_tex.is_some() {
        flat.model.tex_coords = Some(std::sync::Arc::new(tex_coords));
    }
    Ok(flat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genetics::PlantGenotype;
    use crate::interpret::generate_plant_at;
    use crate::lod::LodTier;
    use fyrox::scene::mesh::buffer::{VertexAttributeUsage, VertexReadTrait};
    use rand::SeedableRng;

    fn plant(seed: u64, lod: LodTier) -> PlantModel {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let genotype = PlantGenotype::random_wild(&mut rng);
        generate_plant_at(&genotype, &mut rng, lod).expect("a plant")
    }

    #[test]
    fn a_batch_converts_triangle_for_triangle() {
        let model = plant(42, LodTier::Hub);
        for batch in model.batches().unwrap() {
            let data = to_surface_data(&batch.mesh).unwrap();
            assert_eq!(
                data.geometry_buffer.len(),
                batch.mesh.face_count(),
                "a triangle was lost or invented"
            );
            assert!(data.vertex_buffer.vertex_count() > 0);
        }
    }

    /// The repack must not move a vertex: every position in the buffer has to
    /// be one the source mesh actually had.
    #[test]
    fn positions_survive_the_repack() {
        let model = plant(999, LodTier::Hub);
        let batch = model.batches().unwrap().remove(0);
        let data = to_surface_data(&batch.mesh).unwrap();

        let source: Vec<[f32; 3]> = batch
            .mesh
            .model
            .points
            .iter()
            .map(|p| [p.x, p.y, p.z])
            .collect();
        for vertex in data.vertex_buffer.iter() {
            let p: Vector3<f32> = vertex.read_3_f32(VertexAttributeUsage::Position).unwrap();
            assert!(
                source.contains(&[p.x, p.y, p.z]),
                "{p:?} is not a vertex of the source mesh"
            );
        }
    }

    #[test]
    fn every_vertex_gets_a_unit_normal() {
        let model = plant(100, LodTier::Hub);
        for batch in model.batches().unwrap() {
            let data = to_surface_data(&batch.mesh).unwrap();
            for vertex in data.vertex_buffer.iter() {
                let n: Vector3<f32> = vertex.read_3_f32(VertexAttributeUsage::Normal).unwrap();
                assert!(
                    (n.norm() - 1.0).abs() < 1e-3,
                    "normal {n:?} has length {}",
                    n.norm()
                );
            }
        }
    }

    #[test]
    fn a_plant_becomes_one_node_with_a_surface_per_material() {
        let model = plant(42, LodTier::Hub);
        let mut graph = Graph::new();
        let handle = plant_to_node(&model, &mut graph).unwrap();

        let node = &graph[handle];
        assert_eq!(node.name(), "Plant");
        let mesh = node.cast::<fyrox::scene::mesh::Mesh>().expect("a mesh node");
        let batches = model.batches().unwrap();
        assert_eq!(mesh.surfaces().len(), batches.len());
        assert!(
            mesh.surfaces().len() <= 4,
            "{} surfaces — one per organ kind plus the stem is the most there should be",
            mesh.surfaces().len()
        );
    }

    #[test]
    fn a_scene_converts_the_same_way_the_batches_do() {
        let model = plant(12345, LodTier::Hub);
        let mut graph = Graph::new();
        let from_scene = scene_to_node(
            &model.scene,
            &mut graph,
            model.lod().discretize_ctx(),
        )
        .unwrap();
        let from_batches = plant_to_node(&model, &mut graph).unwrap();

        let triangles = |handle: Handle<Node>, graph: &Graph| -> usize {
            graph[handle]
                .cast::<fyrox::scene::mesh::Mesh>()
                .unwrap()
                .surfaces()
                .iter()
                .map(|s| s.data().data_ref().geometry_buffer.len())
                .sum()
        };
        assert_eq!(
            triangles(from_scene, &graph),
            triangles(from_batches, &graph)
        );
    }

    #[test]
    fn a_material_carries_the_appearances_colour() {
        let model = plant(42, LodTier::Hub);
        let batches = model.batches().unwrap();
        let appearance = batches
            .iter()
            .find_map(|b| b.appearance.clone())
            .expect("a batch with an appearance");
        let expected = appearance.base_color();

        let material = to_material(Some(appearance.as_ref()));
        let material = material.data_ref();
        let value = material
            .property_group_ref("properties")
            .expect("the standard property group")
            .property_ref("diffuseColor")
            .expect("a diffuse colour")
            .as_color();
        assert_eq!(
            value,
            Some(Color::from_rgba(expected.red, expected.green, expected.blue, 255))
        );
    }

    #[test]
    fn an_empty_scene_still_builds_a_node() {
        let mut graph = Graph::new();
        let handle = scene_to_node(&Scene::new(), &mut graph, DiscretizeCtx::default()).unwrap();
        let mesh = graph[handle].cast::<fyrox::scene::mesh::Mesh>().unwrap();
        assert!(mesh.surfaces().is_empty());
    }
}
