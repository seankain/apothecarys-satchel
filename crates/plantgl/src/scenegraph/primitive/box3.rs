//! The axis-aligned box.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/box.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use crate::error::{Error, Result};
use crate::math::{Real, Vec3};

use super::is_positive;

/// Upstream's `Box`, renamed because `Box` is a Rust prelude type.
///
/// **`size` is the half-extent, not the extent.** Upstream's discretizer emits
/// corners at `±size.x, ±size.y, ±size.z`, so the default `(0.5, 0.5, 0.5)` is
/// the *unit* cube, not a half-unit one. Reading it as a full extent halves
/// every box in the scene, which is why it is spelled out here and asserted in
/// `discretize`'s tests.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Box3 {
    /// The `Size` field — the half-extent along each axis.
    pub size: Vec3,
}

impl Box3 {
    /// `Box::DEFAULT_SIZE` — half-extents of a unit cube.
    pub const DEFAULT_SIZE: Vec3 = Vec3::new(0.5, 0.5, 0.5);

    /// `Box(size)`.
    pub fn new(size: Vec3) -> Self {
        Self { size }
    }

    /// A cube of the given full edge length.
    pub fn cube(edge: Real) -> Self {
        Self::new(Vec3::repeat(edge / 2.0))
    }

    /// The full extent along each axis — twice [`Box3::size`].
    pub fn extent(&self) -> Vec3 {
        self.size * 2.0
    }

    /// `isValid()` — every half-extent must be positive.
    pub fn is_valid(&self) -> Result<()> {
        if !is_positive(self.size.x) || !is_positive(self.size.y) || !is_positive(self.size.z) {
            return Err(Error::degenerate(format!(
                "box half-extents must all be positive, got {:?}",
                self.size
            )));
        }
        Ok(())
    }
}

impl Default for Box3 {
    fn default() -> Self {
        Self::new(Self::DEFAULT_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn the_default_box_is_the_unit_cube() {
        assert_relative_eq!(Box3::default().extent(), Vec3::repeat(1.0), epsilon = 1e-6);
    }

    #[test]
    fn cube_takes_a_full_edge_length() {
        assert_relative_eq!(Box3::cube(4.0).size, Vec3::repeat(2.0), epsilon = 1e-6);
    }

    #[test]
    fn a_flat_box_is_degenerate() {
        assert!(Box3::default().is_valid().is_ok());
        assert!(Box3::new(Vec3::new(1.0, 0.0, 1.0)).is_valid().is_err());
        assert!(Box3::new(Vec3::new(1.0, -1.0, 1.0)).is_valid().is_err());
    }
}
