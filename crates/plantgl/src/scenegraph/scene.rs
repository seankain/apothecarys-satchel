//! Shapes and scenes — the output container.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/scene/{shape,scene}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream's `Scene` holds `Shape3DPtr`, which admits `Inline` (a scene
//! referencing another scene file) alongside `Shape`. `Inline` is not ported —
//! plants are generated, not loaded from a scene file — so a `Scene` here is
//! a flat list of `Shape`s.

use crate::algo::bbox::{BBoxComputer, BoundingBox};
use crate::error::Result;
use crate::scenegraph::appearance::{Appearance, AppearanceRef};
use crate::scenegraph::geometry::GeometryRef;

/// Upstream's `Shape::NOID` — the id of a shape that has none.
pub const NOID: u32 = u32::MAX;

/// A geometry with an appearance — upstream's `Shape`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shape {
    pub geometry: GeometryRef,
    pub appearance: Option<AppearanceRef>,
    /// The shape's own id, or [`NOID`].
    pub id: u32,
    /// The id of the shape this one was produced from, or [`NOID`]. The
    /// turtle sets it so a harvested organ can be traced back to its axis.
    pub parent_id: u32,
}

impl Shape {
    pub fn new(geometry: GeometryRef) -> Self {
        Self {
            geometry,
            appearance: None,
            id: NOID,
            parent_id: NOID,
        }
    }

    pub fn with_appearance(mut self, appearance: AppearanceRef) -> Self {
        self.appearance = Some(appearance);
        self
    }

    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    pub fn with_parent_id(mut self, parent_id: u32) -> Self {
        self.parent_id = parent_id;
        self
    }

    /// The shape's bounding box, or `None` if it encloses no points.
    pub fn bbox(&self) -> Result<Option<BoundingBox>> {
        BBoxComputer::new().compute(&self.geometry)
    }

    /// The appearance's name, which the OBJ codec uses as a material name.
    pub fn appearance_name(&self) -> Option<&str> {
        self.appearance.as_deref().and_then(Appearance::name)
    }
}

/// A list of shapes — upstream's `Scene`.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scene {
    pub shapes: Vec<Shape>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_shapes(shapes: Vec<Shape>) -> Self {
        Self { shapes }
    }

    pub fn push(&mut self, shape: Shape) -> &mut Self {
        self.shapes.push(shape);
        self
    }

    /// `merge(const ScenePtr&)` — appends another scene's shapes.
    pub fn merge(&mut self, other: Scene) -> &mut Self {
        self.shapes.extend(other.shapes);
        self
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Shape> {
        self.shapes.iter()
    }

    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// The box enclosing every shape, or `None` for an empty scene.
    pub fn bbox(&self) -> Result<Option<BoundingBox>> {
        let mut computer = BBoxComputer::new();
        let mut result: Option<BoundingBox> = None;
        for shape in &self.shapes {
            let Some(bbox) = computer.compute(&shape.geometry)? else {
                continue;
            };
            result = Some(match result {
                Some(acc) => acc.union(&bbox),
                None => bbox,
            });
        }
        Ok(result)
    }

    /// The shape with the given id, if any.
    pub fn find(&self, id: u32) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }
}

impl<'a> IntoIterator for &'a Scene {
    type Item = &'a Shape;
    type IntoIter = std::slice::Iter<'a, Shape>;

    fn into_iter(self) -> Self::IntoIter {
        self.shapes.iter()
    }
}

impl IntoIterator for Scene {
    type Item = Shape;
    type IntoIter = std::vec::IntoIter<Shape>;

    fn into_iter(self) -> Self::IntoIter {
        self.shapes.into_iter()
    }
}

impl FromIterator<Shape> for Scene {
    fn from_iter<T: IntoIterator<Item = Shape>>(iter: T) -> Self {
        Self {
            shapes: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Point3, Vec3};
    use crate::scenegraph::appearance::Material;
    use crate::scenegraph::geometry::Geometry;
    use crate::scenegraph::mesh::TriangleSet;
    use crate::scenegraph::transform::{Transform, Transformed};
    use approx::assert_relative_eq;
    use std::sync::Arc;

    fn triangle() -> GeometryRef {
        Geometry::from(TriangleSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        ))
        .into_ref()
    }

    fn scene() -> Scene {
        let material = Arc::new(Appearance::Material(Material::new("leaf")));
        Scene::from_shapes(vec![
            Shape::new(triangle())
                .with_appearance(material.clone())
                .with_id(1),
            Shape::new(
                Geometry::from(Transformed::new(
                    Transform::Translated(Vec3::new(5.0, 0.0, 0.0)),
                    triangle(),
                ))
                .into_ref(),
            )
            .with_appearance(material)
            .with_id(2)
            .with_parent_id(1),
        ])
    }

    #[test]
    fn a_new_shape_has_no_ids() {
        let shape = Shape::new(triangle());
        assert_eq!(shape.id, NOID);
        assert_eq!(shape.parent_id, NOID);
        assert_eq!(NOID, u32::MAX);
    }

    #[test]
    fn scene_bbox_unions_every_shape() {
        let bbox = scene().bbox().unwrap().unwrap();
        assert_relative_eq!(bbox.lower_left, Point3::origin(), epsilon = 1e-6);
        assert_relative_eq!(bbox.upper_right, Point3::new(6.0, 1.0, 0.0), epsilon = 1e-6);
    }

    #[test]
    fn an_empty_scene_has_no_bbox() {
        assert_eq!(Scene::new().bbox().unwrap(), None);
        assert!(Scene::new().is_empty());
    }

    #[test]
    fn merge_appends() {
        let mut a = scene();
        let b = scene();
        a.merge(b);
        assert_eq!(a.len(), 4);
    }

    #[test]
    fn find_locates_by_id() {
        let s = scene();
        assert_eq!(s.find(2).unwrap().parent_id, 1);
        assert!(s.find(99).is_none());
    }

    #[test]
    fn appearance_name_reaches_the_material() {
        assert_eq!(scene().shapes[0].appearance_name(), Some("leaf"));
        assert_eq!(Shape::new(triangle()).appearance_name(), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn scene_serde_round_trips() {
        let original = scene();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Scene = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }
}
