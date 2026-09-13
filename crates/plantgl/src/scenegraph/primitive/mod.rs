//! Parametric primitives.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/scenegraph/geometry/{box,sphere,
//! cone,cylinder,frustum,disc,paraboloid,revolution,swung,sor,extrusion,
//! elevationgrid}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # Density is optional here, mandatory upstream
//!
//! Upstream stores `slices` and `stacks` as plain `uchar_t` fields that default
//! to 8, so every primitive pins its own tessellation density at construction
//! and there is no way to re-render a built scene at another quality tier.
//!
//! The port makes them `Option<u8>`: `None` defers to
//! [`DiscretizeCtx`](crate::algo::discretize::DiscretizeCtx), the single LOD
//! knob #21 needs, and `Some(n)` pins the value exactly as upstream does. The
//! constructors that mirror upstream's signatures — [`Sphere::new`] and its
//! siblings — take a plain `u8` and therefore pin it; [`Default`] leaves it
//! `None`. Since `DiscretizeCtx::default()` carries upstream's own defaults, a
//! default-constructed primitive discretises to upstream's mesh either way.

mod box3;
mod cone;
mod disc;
mod elevation_grid;
mod extrusion;
mod revolution;
mod sphere;

pub use box3::Box3;
pub use cone::{Cone, Cylinder, Frustum, Paraboloid};
pub use disc::Disc;
pub use elevation_grid::{ElevationGrid, HeightField};
pub use extrusion::Extrusion;
pub use revolution::{Revolution, Swung};
pub use sphere::Sphere;

use crate::math::Real;

/// `SOR::DEFAULT_SLICES` — the number of subdivisions around the axis of
/// revolution, shared by every surface of revolution.
pub const DEFAULT_SLICES: u8 = 8;

/// The minimum a surface of revolution can be built from: fewer than three
/// slices encloses no volume. Upstream's `SOR::Builder::isValid` requires
/// `slices > 2`.
pub const MIN_SLICES: u8 = 3;

/// The minimum number of stacks along the axis, from upstream's
/// `Sphere::Builder::isValid` and `Paraboloid::Builder::isValid`.
pub const MIN_STACKS: u8 = 2;

/// Resolves a primitive's own density against the context's fallback, then
/// clamps to the minimum the primitive can actually be built from.
#[inline]
pub(crate) fn resolve(own: Option<u8>, fallback: u8, minimum: u8) -> u8 {
    own.unwrap_or(fallback).max(minimum)
}

/// Whether a radius, height or spacing is usable — upstream's `isValid`
/// checks every dimension for `> GEOM_EPSILON` before building.
#[inline]
pub(crate) fn is_positive(value: Real) -> bool {
    value.is_finite() && value > crate::math::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_density_wins_over_the_context() {
        assert_eq!(resolve(Some(32), 8, MIN_SLICES), 32);
    }

    #[test]
    fn no_explicit_density_falls_back_to_the_context() {
        assert_eq!(resolve(None, 64, MIN_SLICES), 64);
    }

    #[test]
    fn both_are_clamped_to_the_primitives_minimum() {
        assert_eq!(resolve(Some(1), 8, MIN_SLICES), MIN_SLICES);
        assert_eq!(resolve(None, 0, MIN_STACKS), MIN_STACKS);
    }

    #[test]
    fn dimensions_must_be_finite_and_positive() {
        assert!(is_positive(1.0));
        assert!(!is_positive(0.0));
        assert!(!is_positive(-1.0));
        assert!(!is_positive(Real::NAN));
        assert!(!is_positive(Real::INFINITY));
    }
}
