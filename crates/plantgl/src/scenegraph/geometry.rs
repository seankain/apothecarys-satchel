//! The geometry sum type and its visitor.
//!
//! Replaces PlantGL `src/cpp/plantgl/scenegraph/core/action.h` and
//! `geometry/geometry.h` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream dispatches through an `Action` interface with one virtual
//! `process(T*)` per primitive, because C++ has no sum types — 41 virtuals per
//! algorithm, and adding a primitive silently compiles against every existing
//! action. Our primitive set is closed, so `enum` + `match` says the same
//! thing: exhaustive at compile time, no vtable, and a fraction of the code.
//! `GeometryVisitor` survives for open-ended traversal.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::scenegraph::curve::{BezierCurve, BezierPatch, NurbsCurve, NurbsPatch};
use crate::scenegraph::mesh::{FaceSet, Group, PointSet, Polyline, QuadSet, TriangleSet};
use crate::scenegraph::primitive::{
    Box3, Cone, Cylinder, Disc, ElevationGrid, Extrusion, Frustum, Paraboloid, Revolution, Sphere,
    Swung,
};
use crate::scenegraph::transform::Transformed;

/// Shared geometry handle.
///
/// Upstream's `GeometryPtr` is an intrusively refcounted `RCPtr`, which is
/// non-atomic and would force `!Send` handles; `Arc` costs one atomic per
/// clone and keeps the scene shareable across threads.
pub type GeometryRef = Arc<Geometry>;

/// Every geometry this crate can carry.
///
/// Variants whose primitive is not yet ported hold a placeholder — see
/// [`crate::scenegraph::primitive`] and [`crate::scenegraph::curve`]. They
/// exist now so the enum's shape does not churn when later phases fill them
/// in, and every visitor already handles them by returning
/// [`Error::Unsupported`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Geometry {
    // Parametric primitives — Phase B (#18).
    Box(Box3),
    Sphere(Sphere),
    Cone(Cone),
    Cylinder(Cylinder),
    Frustum(Frustum),
    Disc(Disc),
    Paraboloid(Paraboloid),
    Revolution(Revolution),
    Swung(Swung),
    ElevationGrid(ElevationGrid),

    // Curves and patches — Phase C (#19).
    BezierCurve(BezierCurve),
    NurbsCurve(NurbsCurve),
    BezierPatch(BezierPatch),
    NurbsPatch(NurbsPatch),
    /// The generalized cylinder — Phase C (#19).
    Extrusion(Extrusion),

    // Explicit models — ported.
    TriangleSet(TriangleSet),
    QuadSet(QuadSet),
    FaceSet(FaceSet),
    PointSet(PointSet),
    Polyline(Polyline),

    // Composition — ported.
    Group(Group),
    Transformed(Box<Transformed>),
}

impl Geometry {
    /// Wraps in an [`Arc`], the form scenes and groups hold.
    pub fn into_ref(self) -> GeometryRef {
        Arc::new(self)
    }

    /// The upstream type name, for error messages and OBJ group names.
    pub fn type_name(&self) -> &'static str {
        match self {
            Geometry::Box(_) => "Box",
            Geometry::Sphere(_) => "Sphere",
            Geometry::Cone(_) => "Cone",
            Geometry::Cylinder(_) => "Cylinder",
            Geometry::Frustum(_) => "Frustum",
            Geometry::Disc(_) => "Disc",
            Geometry::Paraboloid(_) => "Paraboloid",
            Geometry::Revolution(_) => "Revolution",
            Geometry::Swung(_) => "Swung",
            Geometry::ElevationGrid(_) => "ElevationGrid",
            Geometry::BezierCurve(_) => "BezierCurve",
            Geometry::NurbsCurve(_) => "NurbsCurve",
            Geometry::BezierPatch(_) => "BezierPatch",
            Geometry::NurbsPatch(_) => "NurbsPatch",
            Geometry::Extrusion(_) => "Extrusion",
            Geometry::TriangleSet(_) => "TriangleSet",
            Geometry::QuadSet(_) => "QuadSet",
            Geometry::FaceSet(_) => "FaceSet",
            Geometry::PointSet(_) => "PointSet",
            Geometry::Polyline(_) => "Polyline",
            Geometry::Group(_) => "Group",
            Geometry::Transformed(_) => "Transformed",
        }
    }

    /// Whether this variant's primitive has been translated yet. `false`
    /// means the variant is one of the Phase B/C placeholders.
    pub fn is_ported(&self) -> bool {
        matches!(
            self,
            Geometry::TriangleSet(_)
                | Geometry::QuadSet(_)
                | Geometry::FaceSet(_)
                | Geometry::PointSet(_)
                | Geometry::Polyline(_)
                | Geometry::Group(_)
                | Geometry::Transformed(_)
        )
    }

    /// Whether this geometry already carries explicit vertices, so no
    /// discretisation is needed — upstream's `ExplicitModel` subtree.
    pub fn is_explicit(&self) -> bool {
        matches!(
            self,
            Geometry::TriangleSet(_)
                | Geometry::QuadSet(_)
                | Geometry::FaceSet(_)
                | Geometry::PointSet(_)
                | Geometry::Polyline(_)
        )
    }

    /// The error a visitor should return for a not-yet-ported variant.
    pub fn unsupported(&self) -> Error {
        Error::unsupported(format!("{} is not ported yet", self.type_name()))
    }
}

/// Open-ended traversal over a geometry tree, standing in for upstream's
/// `Action`.
///
/// Implement [`GeometryVisitor::visit`] for the leaves you care about; the
/// provided [`GeometryVisitor::walk`] handles `Group` and `Transformed`
/// recursion. A visitor that must track accumulated state across a
/// `Transformed` — [`crate::algo::matrix::MatrixComputer`], for one — should
/// recurse itself rather than use `walk`.
pub trait GeometryVisitor {
    type Output;

    /// Handles one node. `walk` only ever calls this on leaves.
    fn visit(&mut self, geometry: &Geometry) -> Result<Self::Output>;

    /// Depth-first traversal that descends through `Group` and `Transformed`
    /// and calls [`GeometryVisitor::visit`] on every leaf, in order.
    fn walk(&mut self, geometry: &Geometry) -> Result<Vec<Self::Output>> {
        match geometry {
            Geometry::Group(group) => {
                let mut outputs = Vec::new();
                for child in &group.geometries {
                    outputs.extend(self.walk(child)?);
                }
                Ok(outputs)
            }
            Geometry::Transformed(transformed) => self.walk(&transformed.child),
            leaf => Ok(vec![self.visit(leaf)?]),
        }
    }
}

impl From<TriangleSet> for Geometry {
    fn from(m: TriangleSet) -> Self {
        Geometry::TriangleSet(m)
    }
}

impl From<QuadSet> for Geometry {
    fn from(m: QuadSet) -> Self {
        Geometry::QuadSet(m)
    }
}

impl From<FaceSet> for Geometry {
    fn from(m: FaceSet) -> Self {
        Geometry::FaceSet(m)
    }
}

impl From<PointSet> for Geometry {
    fn from(m: PointSet) -> Self {
        Geometry::PointSet(m)
    }
}

impl From<Polyline> for Geometry {
    fn from(m: Polyline) -> Self {
        Geometry::Polyline(m)
    }
}

impl From<Group> for Geometry {
    fn from(g: Group) -> Self {
        Geometry::Group(g)
    }
}

impl From<Transformed> for Geometry {
    fn from(t: Transformed) -> Self {
        Geometry::Transformed(Box::new(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Point3, Vec3};
    use crate::scenegraph::transform::Transform;

    fn triangle(offset: f32) -> GeometryRef {
        Geometry::from(TriangleSet::new(
            vec![
                Point3::new(offset, 0.0, 0.0),
                Point3::new(offset + 1.0, 0.0, 0.0),
                Point3::new(offset, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        ))
        .into_ref()
    }

    /// Counts leaves and records the order they were reached in.
    struct LeafNames(Vec<&'static str>);

    impl GeometryVisitor for LeafNames {
        type Output = &'static str;

        fn visit(&mut self, geometry: &Geometry) -> Result<&'static str> {
            let name = geometry.type_name();
            self.0.push(name);
            Ok(name)
        }
    }

    #[test]
    fn walk_descends_groups_and_transforms_in_order() {
        let tree = Geometry::Group(Group::new(vec![
            triangle(0.0),
            Geometry::from(Transformed::new(
                Transform::Translated(Vec3::x()),
                Geometry::Group(Group::new(vec![triangle(2.0), triangle(4.0)])).into_ref(),
            ))
            .into_ref(),
        ]));

        let mut visitor = LeafNames(Vec::new());
        let outputs = visitor.walk(&tree).unwrap();
        assert_eq!(outputs, vec!["TriangleSet"; 3]);
        assert_eq!(visitor.0.len(), 3);
    }

    #[test]
    fn walk_on_a_leaf_visits_just_that_leaf() {
        let mut visitor = LeafNames(Vec::new());
        assert_eq!(visitor.walk(&triangle(0.0)).unwrap(), vec!["TriangleSet"]);
    }

    #[test]
    fn walk_propagates_a_visitor_error() {
        struct Failing;
        impl GeometryVisitor for Failing {
            type Output = ();
            fn visit(&mut self, geometry: &Geometry) -> Result<()> {
                Err(geometry.unsupported())
            }
        }
        let tree = Geometry::Group(Group::new(vec![triangle(0.0)]));
        assert!(matches!(
            Failing.walk(&tree),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn stub_variants_report_themselves_as_unported() {
        let stub = Geometry::Sphere(Sphere::default());
        assert!(!stub.is_ported());
        assert!(!stub.is_explicit());
        assert_eq!(stub.type_name(), "Sphere");
        assert!(matches!(stub.unsupported(), Error::Unsupported(_)));
    }

    #[test]
    fn explicit_models_are_ported_and_explicit() {
        let mesh = triangle(0.0);
        assert!(mesh.is_ported());
        assert!(mesh.is_explicit());
    }

    #[test]
    fn groups_are_ported_but_not_explicit() {
        let group = Geometry::Group(Group::new(vec![triangle(0.0)]));
        assert!(group.is_ported());
        assert!(!group.is_explicit());
    }
}
