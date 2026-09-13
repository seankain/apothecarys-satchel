//! The flat disc.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/disc.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use crate::error::{Error, Result};
use crate::math::Real;

use super::{is_positive, DEFAULT_SLICES};

/// Upstream's `Disc` — a filled circle in the `z = 0` plane, centred on the
/// origin and facing +z.
///
/// Upstream derives it from `SOR2D` rather than `SOR`: it is a planar model,
/// so it has slices but no stacks and no height.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Disc {
    /// The `Radius` field.
    pub radius: Real,
    pub slices: Option<u8>,
}

impl Disc {
    /// `Disc::DEFAULT_RADIUS`. Upstream's other primitives alias their own
    /// default radius to this one.
    pub const DEFAULT_RADIUS: Real = 0.5;
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;

    /// `Disc(radius, slices)`.
    pub fn new(radius: Real, slices: u8) -> Self {
        Self {
            radius,
            slices: Some(slices),
        }
    }

    /// A disc whose density the discretisation context decides.
    pub fn sized(radius: Real) -> Self {
        Self {
            radius,
            slices: None,
        }
    }

    pub fn with_context_slices(mut self) -> Self {
        self.slices = None;
        self
    }

    /// `isValid()`.
    pub fn is_valid(&self) -> Result<()> {
        if !is_positive(self.radius) {
            return Err(Error::degenerate(format!(
                "disc radius must be positive, got {}",
                self.radius
            )));
        }
        Ok(())
    }
}

impl Default for Disc {
    fn default() -> Self {
        Self {
            radius: Self::DEFAULT_RADIUS,
            slices: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn the_default_disc_matches_upstream() {
        assert_relative_eq!(Disc::default().radius, 0.5);
        assert_eq!(Disc::default().slices, None);
        assert_eq!(Disc::new(1.0, 24).slices, Some(24));
    }

    #[test]
    fn a_zero_radius_disc_is_degenerate() {
        assert!(Disc::default().is_valid().is_ok());
        assert!(Disc::sized(0.0).is_valid().is_err());
    }
}
