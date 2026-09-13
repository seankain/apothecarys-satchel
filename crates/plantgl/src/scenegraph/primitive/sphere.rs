//! The sphere.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/sphere.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use crate::error::{Error, Result};
use crate::math::Real;

use super::{is_positive, DEFAULT_SLICES};

/// Upstream's `Sphere`, centred on the origin with its poles on the z axis.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sphere {
    /// The `Radius` field.
    pub radius: Real,
    /// The `Slices` field — meridians, around z.
    pub slices: Option<u8>,
    /// The `Stacks` field — parallels, from pole to pole. A sphere has
    /// `stacks - 1` rings of points plus the two poles.
    pub stacks: Option<u8>,
}

impl Sphere {
    /// `Sphere::DEFAULT_RADIUS`, which upstream aliases to
    /// `Disc::DEFAULT_RADIUS`.
    pub const DEFAULT_RADIUS: Real = 0.5;
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;
    /// `Sphere::DEFAULT_STACKS`.
    pub const DEFAULT_STACKS: u8 = 8;

    /// `Sphere(radius, slices, stacks)`.
    pub fn new(radius: Real, slices: u8, stacks: u8) -> Self {
        Self {
            radius,
            slices: Some(slices),
            stacks: Some(stacks),
        }
    }

    /// A sphere whose density the discretisation context decides.
    pub fn sized(radius: Real) -> Self {
        Self {
            radius,
            slices: None,
            stacks: None,
        }
    }

    pub fn with_context_density(mut self) -> Self {
        self.slices = None;
        self.stacks = None;
        self
    }

    /// `isValid()`.
    pub fn is_valid(&self) -> Result<()> {
        if !is_positive(self.radius) {
            return Err(Error::degenerate(format!(
                "sphere radius must be positive, got {}",
                self.radius
            )));
        }
        Ok(())
    }
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            radius: Self::DEFAULT_RADIUS,
            slices: None,
            stacks: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn the_default_sphere_has_upstreams_radius() {
        assert_relative_eq!(Sphere::default().radius, 0.5);
        assert_eq!(Sphere::default().slices, None);
    }

    #[test]
    fn upstreams_constructor_pins_both_densities() {
        let s = Sphere::new(2.0, 32, 16);
        assert_eq!((s.slices, s.stacks), (Some(32), Some(16)));
        assert_eq!(s.with_context_density().slices, None);
    }

    #[test]
    fn a_zero_radius_sphere_is_degenerate() {
        assert!(Sphere::default().is_valid().is_ok());
        assert!(Sphere::sized(0.0).is_valid().is_err());
        assert!(Sphere::sized(-1.0).is_valid().is_err());
    }
}
