//! Accumulation of transformations down a geometry tree.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/base/matrixcomputer.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! **One deliberate divergence.** Upstream's `MatrixComputer::process(Tapered*)`
//! calls `default_process`, which descends into the child and drops the taper
//! on the floor: a tapered frustum comes back with the matrix of an untapered
//! one. That is defensible there, because `Discretizer` applies the taper
//! separately, but a caller who asks this type for "the transform at this
//! leaf" and gets a silent omission has been misled. The port keeps a
//! [`Deformation`] stack alongside the matrix and reports both.

use crate::error::{Error, Result};
use crate::math::Mat4;
use crate::scenegraph::geometry::{Geometry, GeometryRef};
use crate::scenegraph::transform::{Deformation, Transform};

/// Everything needed to place a leaf in world space: the accumulated affine
/// matrix, plus the deformations that could not be folded into it, outermost
/// first.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub matrix: Mat4,
    pub deformations: Vec<Deformation>,
}

impl Placement {
    /// Whether this placement is a pure affine transform.
    pub fn is_affine(&self) -> bool {
        self.deformations.is_empty()
    }
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            matrix: Mat4::identity(),
            deformations: Vec::new(),
        }
    }
}

/// Upstream's `MatrixComputer` Action.
#[derive(Debug, Clone)]
pub struct MatrixComputer {
    matrix: Mat4,
    stack: Vec<Mat4>,
    deformations: Vec<Deformation>,
    /// Deformation-stack depth saved by each `push`, so `pop` can truncate.
    deformation_depths: Vec<usize>,
}

impl Default for MatrixComputer {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixComputer {
    pub fn new() -> Self {
        Self {
            matrix: Mat4::identity(),
            stack: Vec::new(),
            deformations: Vec::new(),
            deformation_depths: Vec::new(),
        }
    }

    /// `clear()` — back to the identity with an empty stack.
    pub fn clear(&mut self) {
        self.matrix = Mat4::identity();
        self.stack.clear();
        self.deformations.clear();
        self.deformation_depths.clear();
    }

    /// The accumulated affine transform — upstream's `getMatrix()`.
    pub fn matrix(&self) -> &Mat4 {
        &self.matrix
    }

    /// The deformations in effect, outermost first. Upstream has no
    /// equivalent; see the module docs.
    pub fn deformations(&self) -> &[Deformation] {
        &self.deformations
    }

    /// A snapshot of both.
    pub fn placement(&self) -> Placement {
        Placement {
            matrix: self.matrix,
            deformations: self.deformations.clone(),
        }
    }

    /// Saves the current state, as upstream's `pushMatrix`.
    pub fn push(&mut self) {
        self.stack.push(self.matrix);
        self.deformation_depths.push(self.deformations.len());
    }

    /// Restores the state saved by the matching [`MatrixComputer::push`].
    pub fn pop(&mut self) -> Result<()> {
        self.matrix = self.stack.pop().ok_or(Error::EmptyStack)?;
        let depth = self.deformation_depths.pop().ok_or(Error::EmptyStack)?;
        self.deformations.truncate(depth);
        Ok(())
    }

    /// Applies one transform: right-multiplies the matrix, as upstream's
    /// `transfo_process`, or records a deformation when it is not affine.
    pub fn apply(&mut self, transform: &Transform) -> Result<()> {
        if let Some(deformation) = transform.to_deformation() {
            self.deformations.push(deformation);
            return Ok(());
        }
        let matrix = transform.to_matrix4().ok_or_else(|| {
            Error::degenerate(format!("{transform:?} has no transformation matrix"))
        })?;
        self.matrix *= matrix;
        Ok(())
    }

    /// Walks a geometry tree and returns every leaf with the placement in
    /// effect there, depth first.
    ///
    /// `Group` and `Transformed` are the only interior nodes; everything else
    /// is a leaf, including not-yet-ported primitives — placing them is well
    /// defined even though discretising them is not.
    pub fn flatten(&mut self, geometry: &GeometryRef) -> Result<Vec<(Placement, GeometryRef)>> {
        let mut leaves = Vec::new();
        self.flatten_into(geometry, &mut leaves)?;
        Ok(leaves)
    }

    fn flatten_into(
        &mut self,
        geometry: &GeometryRef,
        leaves: &mut Vec<(Placement, GeometryRef)>,
    ) -> Result<()> {
        match geometry.as_ref() {
            Geometry::Group(group) => {
                for child in &group.geometries {
                    self.flatten_into(child, leaves)?;
                }
            }
            Geometry::Transformed(transformed) => {
                self.push();
                let result = self
                    .apply(&transformed.transform)
                    .and_then(|()| self.flatten_into(&transformed.child, leaves));
                self.pop()?;
                result?;
            }
            _ => leaves.push((self.placement(), geometry.clone())),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Point3, Vec3};
    use crate::scenegraph::mesh::{Group, TriangleSet};
    use crate::scenegraph::primitive::Sphere;
    use crate::scenegraph::transform::{Taper, Transformed};
    use approx::assert_relative_eq;

    fn sphere() -> GeometryRef {
        Geometry::Sphere(Sphere::default()).into_ref()
    }

    fn transformed(transform: Transform, child: GeometryRef) -> GeometryRef {
        Geometry::from(Transformed::new(transform, child)).into_ref()
    }

    /// Acceptance (T8.2): nested `Translated(Scaled(Sphere))` yields the
    /// expected 4×4.
    #[test]
    fn nested_translate_of_scale_composes_outermost_first() {
        let tree = transformed(
            Transform::Translated(Vec3::new(1.0, 2.0, 3.0)),
            transformed(Transform::Scaled(Vec3::new(2.0, 2.0, 2.0)), sphere()),
        );

        let mut computer = MatrixComputer::new();
        let leaves = computer.flatten(&tree).unwrap();
        assert_eq!(leaves.len(), 1);

        let expected = Mat4::new_translation(&Vec3::new(1.0, 2.0, 3.0))
            * Mat4::new_nonuniform_scaling(&Vec3::new(2.0, 2.0, 2.0));
        assert_relative_eq!(leaves[0].0.matrix, expected, epsilon = 1e-6);
        assert!(leaves[0].0.is_affine());

        // A unit-x point scales to (2,0,0) then translates to (3,2,3).
        let placed = leaves[0].0.matrix * Point3::new(1.0, 0.0, 0.0).to_homogeneous();
        assert_relative_eq!(
            Point3::from_homogeneous(placed).unwrap(),
            Point3::new(3.0, 2.0, 3.0),
            epsilon = 1e-6
        );
    }

    /// Acceptance (T8.2): a `Tapered` is reported as a deformation rather
    /// than folded into the matrix.
    #[test]
    fn tapered_is_reported_not_folded_in() {
        let tree = transformed(
            Transform::Translated(Vec3::new(0.0, 0.0, 5.0)),
            transformed(
                Transform::Tapered {
                    base_radius: 1.0,
                    top_radius: 0.25,
                },
                sphere(),
            ),
        );

        let mut computer = MatrixComputer::new();
        let leaves = computer.flatten(&tree).unwrap();
        assert_eq!(leaves.len(), 1);

        let placement = &leaves[0].0;
        assert!(!placement.is_affine());
        assert_eq!(
            placement.deformations,
            vec![Deformation::Taper(Taper::new(1.0, 0.25))]
        );
        // The translation is still there, and nothing else is.
        assert_relative_eq!(
            placement.matrix,
            Mat4::new_translation(&Vec3::new(0.0, 0.0, 5.0)),
            epsilon = 1e-6
        );
    }

    #[test]
    fn siblings_do_not_inherit_each_others_transforms() {
        let tree = Geometry::Group(Group::new(vec![
            transformed(Transform::Translated(Vec3::x()), sphere()),
            sphere(),
        ]))
        .into_ref();

        let mut computer = MatrixComputer::new();
        let leaves = computer.flatten(&tree).unwrap();
        assert_eq!(leaves.len(), 2);
        assert_relative_eq!(
            leaves[0].0.matrix,
            Mat4::new_translation(&Vec3::x()),
            epsilon = 1e-6
        );
        assert_relative_eq!(leaves[1].0.matrix, Mat4::identity(), epsilon = 1e-6);
        assert_relative_eq!(*computer.matrix(), Mat4::identity(), epsilon = 1e-6);
    }

    #[test]
    fn a_deformation_does_not_leak_to_a_sibling() {
        let tree = Geometry::Group(Group::new(vec![
            transformed(
                Transform::Tapered {
                    base_radius: 1.0,
                    top_radius: 0.5,
                },
                sphere(),
            ),
            sphere(),
        ]))
        .into_ref();

        let mut computer = MatrixComputer::new();
        let leaves = computer.flatten(&tree).unwrap();
        assert_eq!(leaves[0].0.deformations.len(), 1);
        assert!(leaves[1].0.is_affine());
        assert!(computer.deformations().is_empty());
    }

    #[test]
    fn explicit_leaves_come_back_as_leaves() {
        let mesh = Geometry::from(TriangleSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        ))
        .into_ref();
        let mut computer = MatrixComputer::new();
        let leaves = computer.flatten(&mesh).unwrap();
        assert_eq!(leaves.len(), 1);
        assert!(matches!(*leaves[0].1, Geometry::TriangleSet(_)));
    }

    #[test]
    fn a_degenerate_transform_is_an_error_and_unwinds_the_stack() {
        let tree = transformed(
            Transform::AxisRotated {
                axis: Vec3::zeros(),
                angle: 1.0,
            },
            sphere(),
        );
        let mut computer = MatrixComputer::new();
        assert!(matches!(
            computer.flatten(&tree),
            Err(Error::DegenerateGeometry(_))
        ));
        assert_relative_eq!(*computer.matrix(), Mat4::identity(), epsilon = 1e-6);
    }

    #[test]
    fn pop_without_push_is_an_empty_stack_error() {
        let mut computer = MatrixComputer::new();
        assert_eq!(computer.pop(), Err(Error::EmptyStack));
    }

    #[test]
    fn clear_resets_everything() {
        let mut computer = MatrixComputer::new();
        computer.push();
        computer.apply(&Transform::Translated(Vec3::x())).unwrap();
        computer
            .apply(&Transform::Tapered {
                base_radius: 1.0,
                top_radius: 0.5,
            })
            .unwrap();
        computer.clear();
        assert_relative_eq!(*computer.matrix(), Mat4::identity(), epsilon = 1e-6);
        assert!(computer.deformations().is_empty());
        assert_eq!(computer.pop(), Err(Error::EmptyStack));
    }
}
