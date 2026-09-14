//! The garden scene: a grid of plots, each with the plant it is growing.
//!
//! This is the spawn path #21 moved onto `plantgl`. A plot's plant is grown
//! from its genotype through `apothecarys_botany::interpret` and handed to the
//! renderer by `apothecarys_botany::fyrox_bridge`, merged by appearance so one
//! plant is a handful of draw calls rather than one per leaf.
//!
//! # Determinism
//!
//! A plant has to look the same every time the garden is entered, and the same
//! after a save is reloaded. The L-system's only randomness is its choice of
//! production, so the whole appearance is a function of the seed — and the
//! seed here is the plant's own [`Uuid`], which is stored in the save. Nothing
//! about the plant's look is derived from wall-clock time or from the order
//! plots happen to be visited in.
//!
//! # Level of detail
//!
//! Plots are built once, at garden load and on harvest, not per frame, so the
//! tier is chosen from the plot's distance to the camera at build time. The
//! near plots get [`LodTier::Hub`]; the far ones get [`LodTier::Distant`],
//! which is roughly an eighth of the triangles.

use apothecarys_botany::fyrox_bridge::plant_to_node;
use apothecarys_botany::interpret::generate_plant_at;
use apothecarys_botany::lod::LodTier;
use apothecarys_garden::plots::{Garden, PlantInstance, PlotState};
use fyrox::{
    asset::untyped::ResourceKind,
    core::{
        algebra::{Matrix4, UnitQuaternion, Vector3},
        color::Color,
        log::{Log, MessageKind},
        pool::Handle,
    },
    material::{Material, MaterialResource},
    scene::{
        base::BaseBuilder,
        graph::Graph,
        light::{directional::DirectionalLightBuilder, BaseLightBuilder},
        mesh::{
            surface::{SurfaceBuilder, SurfaceData, SurfaceResource},
            MeshBuilder, RenderPath,
        },
        node::Node,
        transform::TransformBuilder,
        Scene,
    },
};
use rand::SeedableRng;

/// How the plots are laid out on the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GardenLayout {
    /// World units between plot centres.
    pub spacing: f32,
    /// Plots per row before wrapping.
    pub columns: usize,
    /// Where plot zero sits.
    pub origin: Vector3<f32>,
    /// Beyond this distance from the camera a plot is built at
    /// [`LodTier::Distant`].
    pub near_distance: f32,
}

impl Default for GardenLayout {
    fn default() -> Self {
        Self {
            spacing: 2.5,
            columns: 4,
            origin: Vector3::new(0.0, 0.0, 0.0),
            near_distance: 12.0,
        }
    }
}

impl GardenLayout {
    /// Where plot `index` sits, centring the grid on the origin.
    pub fn plot_position(&self, index: usize) -> Vector3<f32> {
        let columns = self.columns.max(1);
        let (column, row) = (index % columns, index / columns);
        let half = (columns as f32 - 1.0) * 0.5;
        self.origin
            + Vector3::new(
                (column as f32 - half) * self.spacing,
                0.0,
                row as f32 * self.spacing,
            )
    }

    /// The tier a plot at `position` is built at, seen from `viewer`.
    pub fn tier_for(&self, position: Vector3<f32>, viewer: Vector3<f32>) -> LodTier {
        if (position - viewer).norm() <= self.near_distance {
            LodTier::Hub
        } else {
            LodTier::Distant
        }
    }
}

/// The seed a plant's appearance is generated from.
///
/// Its `Uuid` is stored in the save, so the plant the player saw yesterday is
/// the plant they see today.
pub fn plant_seed(plant: &PlantInstance) -> u64 {
    plant.id.as_u128() as u64
}

/// How much of its full size a plant has reached at `growth_stage`.
///
/// A seedling is a quarter-height version of what it will become rather than
/// nothing at all, so an unwatered plot still reads as planted.
pub fn growth_scale(growth_stage: f32) -> f32 {
    0.25 + 0.75 * growth_stage.clamp(0.0, 1.0)
}

/// Builds one plant node at `position`, scaled for its growth stage.
pub fn spawn_plant(
    plant: &PlantInstance,
    growth_stage: f32,
    position: Vector3<f32>,
    tier: LodTier,
    graph: &mut Graph,
) -> Result<Handle<Node>, plantgl::Error> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(plant_seed(plant));
    let model = generate_plant_at(&plant.genotype, &mut rng, tier)?;
    let handle = plant_to_node(&model, graph)?;

    let scale = growth_scale(growth_stage);
    graph[handle]
        .local_transform_mut()
        .set_position(position)
        .set_scale(Vector3::new(scale, scale, scale));
    graph[handle].set_name(format!("Plant_{}", plant.id));
    Ok(handle)
}

/// Builds every planted plot in `garden`, returning a handle per plot in plot
/// order — `Handle::NONE` for an empty plot, so the caller can index by plot.
///
/// A plant that fails to build is logged and skipped rather than taking the
/// garden down with it: one malformed genotype should cost one plot.
pub fn spawn_garden(
    garden: &Garden,
    layout: &GardenLayout,
    viewer: Vector3<f32>,
    graph: &mut Graph,
) -> Vec<Handle<Node>> {
    garden
        .plots
        .iter()
        .map(|plot| {
            let PlotState::Planted {
                plant,
                growth_stage,
                ..
            } = &plot.state
            else {
                return Handle::NONE;
            };
            let position = layout.plot_position(plot.index);
            let tier = layout.tier_for(position, viewer);
            match spawn_plant(plant, *growth_stage, position, tier, graph) {
                Ok(handle) => handle,
                Err(e) => {
                    Log::writeln(
                        MessageKind::Error,
                        format!("Plot {}: could not build {}: {e}", plot.index, plant.species_name),
                    );
                    Handle::NONE
                }
            }
        })
        .collect()
}

/// The whole garden as a scene: ground, light, plot beds and plants.
///
/// Returns the scene and the per-plot node handles, in plot order.
pub fn build_garden_scene(
    garden: &Garden,
    layout: &GardenLayout,
    viewer: Vector3<f32>,
) -> (Scene, Vec<Handle<Node>>) {
    let mut scene = Scene::new();
    scene.rendering_options.ambient_lighting_color = Color::from_rgba(150, 195, 225, 255);

    DirectionalLightBuilder::new(
        BaseLightBuilder::new(
            BaseBuilder::new()
                .with_name("Sun")
                .with_local_transform(
                    TransformBuilder::new()
                        .with_local_rotation(UnitQuaternion::face_towards(
                            &Vector3::new(-0.5, -1.0, -0.3).normalize(),
                            &Vector3::new(0.0, 1.0, 0.0),
                        ))
                        .build(),
                ),
        )
        .with_color(Color::from_rgba(255, 248, 230, 255)),
    )
    .build(&mut scene.graph);

    build_ground(&mut scene.graph);
    for plot in &garden.plots {
        build_plot_bed(&mut scene.graph, layout.plot_position(plot.index), plot.index);
    }

    let plants = spawn_garden(garden, layout, viewer, &mut scene.graph);
    (scene, plants)
}

fn colored_material(color: Color) -> MaterialResource {
    let mut material = Material::standard();
    material.set_property("diffuseColor", color);
    MaterialResource::new_ok(ResourceKind::Embedded, material)
}

fn build_ground(graph: &mut Graph) -> Handle<Node> {
    let surface_data = SurfaceData::make_quad(&Matrix4::new_nonuniform_scaling(&Vector3::new(
        50.0, 1.0, 50.0,
    )));

    MeshBuilder::new(
        BaseBuilder::new()
            .with_name("GroundPlane")
            .with_local_transform(
                TransformBuilder::new()
                    // Rotate the XY quad so it lies flat on XZ.
                    .with_local_rotation(UnitQuaternion::from_axis_angle(
                        &Vector3::x_axis(),
                        -std::f32::consts::FRAC_PI_2,
                    ))
                    .build(),
            ),
    )
    .with_surfaces(vec![SurfaceBuilder::new(SurfaceResource::new_ok(
        ResourceKind::Embedded,
        surface_data,
    ))
    .with_material(colored_material(Color::opaque(96, 128, 72)))
    .build()])
    .with_render_path(RenderPath::Forward)
    .build(graph)
}

/// A shallow bed of tilled soil under each plot, so an empty plot still reads
/// as a plot.
fn build_plot_bed(graph: &mut Graph, position: Vector3<f32>, index: usize) -> Handle<Node> {
    let surface_data = SurfaceData::make_cube(Matrix4::new_nonuniform_scaling(&Vector3::new(
        0.9, 0.06, 0.9,
    )));

    MeshBuilder::new(
        BaseBuilder::new()
            .with_name(format!("PlotBed_{index}"))
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_position(position + Vector3::new(0.0, 0.03, 0.0))
                    .build(),
            ),
    )
    .with_surfaces(vec![SurfaceBuilder::new(SurfaceResource::new_ok(
        ResourceKind::Embedded,
        surface_data,
    ))
    .with_material(colored_material(Color::opaque(84, 62, 44)))
    .build()])
    .with_render_path(RenderPath::Forward)
    .build(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_botany::genetics::PlantGenotype;
    use fyrox::graph::SceneGraph;
    use rand::SeedableRng;

    fn garden_with(planted: usize) -> Garden {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let mut garden = Garden::new(6);
        for index in 0..planted {
            let plant = PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Test Herb");
            garden.get_plot_mut(index).unwrap().plant_seed(plant).unwrap();
        }
        garden
    }

    #[test]
    fn plots_lay_out_in_a_centred_grid() {
        let layout = GardenLayout::default();
        let first = layout.plot_position(0);
        let last_in_row = layout.plot_position(layout.columns - 1);
        assert!((first.x + last_in_row.x).abs() < 1e-5, "the row is not centred");
        assert_eq!(first.z, last_in_row.z, "the row is not a row");
        // The next row is one spacing further back.
        let next_row = layout.plot_position(layout.columns);
        assert!((next_row.z - first.z - layout.spacing).abs() < 1e-5);
    }

    #[test]
    fn distance_picks_the_tier() {
        let layout = GardenLayout::default();
        let viewer = Vector3::new(0.0, 0.0, 0.0);
        assert_eq!(layout.tier_for(Vector3::new(1.0, 0.0, 1.0), viewer), LodTier::Hub);
        assert_eq!(
            layout.tier_for(Vector3::new(0.0, 0.0, layout.near_distance + 1.0), viewer),
            LodTier::Distant
        );
    }

    #[test]
    fn a_seedling_is_smaller_than_a_mature_plant() {
        assert!(growth_scale(0.0) > 0.0);
        assert!(growth_scale(0.0) < growth_scale(0.5));
        assert!(growth_scale(0.5) < growth_scale(1.0));
        assert!((growth_scale(1.0) - 1.0).abs() < 1e-6);
        // Out of range input clamps rather than inverting the plant.
        assert_eq!(growth_scale(-1.0), growth_scale(0.0));
        assert_eq!(growth_scale(2.0), growth_scale(1.0));
    }

    #[test]
    fn the_garden_spawns_one_node_per_planted_plot() {
        let garden = garden_with(3);
        let (scene, plants) = build_garden_scene(
            &garden,
            &GardenLayout::default(),
            Vector3::new(0.0, 0.0, 0.0),
        );

        assert_eq!(plants.len(), garden.plots.len());
        assert_eq!(plants.iter().filter(|h| h.is_some()).count(), 3);
        for (index, handle) in plants.iter().enumerate() {
            if handle.is_none() {
                continue;
            }
            let node = &scene.graph[*handle];
            let position = **node.local_transform().position();
            let expected = GardenLayout::default().plot_position(index);
            assert!((position - expected).norm() < 1e-5, "plot {index} is misplaced");
            assert!(node.name().starts_with("Plant_"));
        }
    }

    #[test]
    fn an_empty_garden_still_builds_its_beds() {
        let garden = garden_with(0);
        let (scene, plants) = build_garden_scene(
            &garden,
            &GardenLayout::default(),
            Vector3::new(0.0, 0.0, 0.0),
        );
        assert!(plants.iter().all(|h| h.is_none()));
        let beds = scene
            .graph
            .pair_iter()
            .filter(|(_, node): &(Handle<Node>, &Node)| node.name().starts_with("PlotBed_"))
            .count();
        assert_eq!(beds, garden.plots.len());
    }

    /// The same plant has to look the same every time the garden is entered.
    #[test]
    fn a_plant_is_the_same_every_time_it_is_spawned() {
        let garden = garden_with(1);
        let viewer = Vector3::new(0.0, 0.0, 0.0);

        let triangles = |scene: &Scene, handle: Handle<Node>| -> usize {
            scene.graph[handle]
                .cast::<fyrox::scene::mesh::Mesh>()
                .unwrap()
                .surfaces()
                .iter()
                .map(|s| s.data().data_ref().geometry_buffer.len())
                .sum()
        };

        let (first, first_plants) =
            build_garden_scene(&garden, &GardenLayout::default(), viewer);
        let (second, second_plants) =
            build_garden_scene(&garden, &GardenLayout::default(), viewer);
        assert_eq!(
            triangles(&first, first_plants[0]),
            triangles(&second, second_plants[0])
        );
    }

    /// A distant plot is built coarser than a near one, which is the whole
    /// point of choosing the tier at spawn time.
    #[test]
    fn a_distant_plot_is_cheaper_than_a_near_one() {
        let garden = garden_with(1);
        let plant = garden.plots[0].state.plant().unwrap();

        let mut graph = Graph::new();
        let near = spawn_plant(plant, 1.0, Vector3::zeros(), LodTier::Hub, &mut graph).unwrap();
        let far = spawn_plant(plant, 1.0, Vector3::zeros(), LodTier::Distant, &mut graph).unwrap();

        let triangles = |handle: Handle<Node>, graph: &Graph| -> usize {
            graph[handle]
                .cast::<fyrox::scene::mesh::Mesh>()
                .unwrap()
                .surfaces()
                .iter()
                .map(|s| s.data().data_ref().geometry_buffer.len())
                .sum()
        };
        assert!(triangles(near, &graph) > triangles(far, &graph));
    }

    #[test]
    fn growth_stage_scales_the_plant() {
        let garden = garden_with(1);
        let plant = garden.plots[0].state.plant().unwrap();

        let mut graph = Graph::new();
        let seedling = spawn_plant(plant, 0.0, Vector3::zeros(), LodTier::Hub, &mut graph).unwrap();
        let mature = spawn_plant(plant, 1.0, Vector3::zeros(), LodTier::Hub, &mut graph).unwrap();

        assert!(
            graph[seedling].local_transform().scale().y < graph[mature].local_transform().scale().y
        );
    }
}
