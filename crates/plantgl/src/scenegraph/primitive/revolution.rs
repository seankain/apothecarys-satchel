//! Surfaces swept from a 2D profile: the surface of revolution and the swung
//! surface.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{revolution,swung,
//! profile}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use crate::error::{Error, Result};
use crate::math::Real;
use crate::scenegraph::curve::Curve2DRef;

use super::DEFAULT_SLICES;

/// Upstream's `Revolution` — one 2D profile swept a full turn about the z
/// axis.
///
/// In the profile, `x` is the radius from the axis and `y` the height along
/// it, so a profile of `[(1, 0), (1, 2)]` sweeps to a cylinder of radius 1 and
/// height 2.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Revolution {
    /// The `Profile` field.
    pub profile: Curve2DRef,
    pub slices: Option<u8>,
}

impl Revolution {
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;

    /// `Revolution(profile, slices)`.
    pub fn new(profile: Curve2DRef, slices: u8) -> Self {
        Self {
            profile,
            slices: Some(slices),
        }
    }

    /// A revolution whose density the discretisation context decides.
    pub fn from_profile(profile: Curve2DRef) -> Self {
        Self {
            profile,
            slices: None,
        }
    }

    pub fn with_context_slices(mut self) -> Self {
        self.slices = None;
        self
    }

    /// `isValid()` — the profile must sample to at least two points and stay
    /// on the non-negative side of the axis.
    ///
    /// `curve_samples` is the density a parametric profile is evaluated at;
    /// an explicit profile ignores it.
    pub fn is_valid(&self, curve_samples: u32) -> Result<()> {
        let points = self.profile.sample(curve_samples)?;
        if points.len() < 2 {
            return Err(Error::degenerate(
                "a revolution profile needs at least 2 points",
            ));
        }
        if points.iter().any(|p| p.x < 0.0) {
            return Err(Error::degenerate(
                "a revolution profile must not cross the axis of revolution",
            ));
        }
        Ok(())
    }
}

/// Upstream's `Swung` — several 2D profiles, each pinned to an angle about the
/// z axis, interpolated between as the sweep goes round.
///
/// A single profile degenerates to a [`Revolution`]. Several let a stem change
/// cross-section as it turns, which is how upstream builds ridged and fluted
/// organs.
///
/// # Interpolation degree
///
/// Upstream's `ProfileInterpolation` fits, for each sample position along the
/// profile, a NURBS curve of `degree` through that position's value in every
/// profile, and evaluates it at the sweep angle. Degree 1 is piecewise-linear
/// blending and is what this phase translates; degrees 2 and 3 need the global
/// NURBS interpolation that lands with the rest of the spline machinery in
/// Phase C (#19), and `discretize` reports them as [`Error::Unsupported`]
/// until then rather than silently blending linearly and producing a subtly
/// wrong surface.
///
/// [`Error::Unsupported`]: crate::Error::Unsupported
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Swung {
    /// The `ProfileList` field.
    pub profiles: Vec<Curve2DRef>,
    /// The `AngleList` field — one angle in radians per profile, ascending.
    pub angles: Vec<Real>,
    pub slices: Option<u8>,
    /// The `CCW` field — the winding the swept triangles are built with.
    pub ccw: bool,
    /// The `Degree` field of the profile interpolation.
    pub degree: u32,
    /// The `Stride` field — how many segments each profile is resampled to
    /// before blending. `0` means "take the coarsest profile's own stride",
    /// as upstream's `ProfileInterpolation::interpol` does.
    pub stride: u32,
}

impl Swung {
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;
    /// `Swung::DEFAULT_CCW`.
    pub const DEFAULT_CCW: bool = true;
    /// `Swung::DEFAULT_DEGREE`, from `ProfileInterpolation::DEFAULT_DEGREE`.
    pub const DEFAULT_DEGREE: u32 = 3;
    /// `Swung::DEFAULT_STRIDE`, from `ProfileInterpolation::DEFAULT_STRIDE`.
    pub const DEFAULT_STRIDE: u32 = 0;
    /// The highest interpolation degree this phase implements; see the type
    /// docs.
    pub const MAX_PORTED_DEGREE: u32 = 1;

    /// `Swung(profileList, angleList, slices, ccw, degree, stride)`.
    pub fn new(
        profiles: Vec<Curve2DRef>,
        angles: Vec<Real>,
        slices: u8,
        ccw: bool,
        degree: u32,
        stride: u32,
    ) -> Self {
        Self {
            profiles,
            angles,
            slices: Some(slices),
            ccw,
            degree,
            stride,
        }
    }

    /// Profiles blended linearly, at the density the discretisation context
    /// decides — the configuration this phase can discretise.
    pub fn linear(profiles: Vec<Curve2DRef>, angles: Vec<Real>) -> Self {
        Self {
            profiles,
            angles,
            slices: None,
            ccw: Self::DEFAULT_CCW,
            degree: 1,
            stride: Self::DEFAULT_STRIDE,
        }
    }

    pub fn with_context_slices(mut self) -> Self {
        self.slices = None;
        self
    }

    /// `isValid()` — the lists must be the same non-empty length, the angles
    /// strictly ascending, and every profile a usable curve.
    pub fn is_valid(&self, curve_samples: u32) -> Result<()> {
        if self.profiles.is_empty() {
            return Err(Error::degenerate("a swung surface needs at least 1 profile"));
        }
        if self.profiles.len() != self.angles.len() {
            return Err(Error::invalid_index(format!(
                "swung has {} profiles and {} angles",
                self.profiles.len(),
                self.angles.len()
            )));
        }
        if self.angles.windows(2).any(|w| w[1] <= w[0]) {
            return Err(Error::degenerate(
                "swung angles must be strictly ascending",
            ));
        }
        let mut expected: Option<usize> = None;
        for profile in &self.profiles {
            let points = profile.sample(curve_samples)?;
            if points.len() < 2 {
                return Err(Error::degenerate(
                    "a swung profile needs at least 2 points",
                ));
            }
            if points.iter().any(|p| p.x < 0.0) {
                return Err(Error::degenerate(
                    "a swung profile must not cross the axis of revolution",
                ));
            }
            // Upstream resamples every profile to `stride + 1` points before
            // blending, so profiles of different lengths are legal there. We
            // resample too, so this only rejects what resampling cannot fix.
            expected.get_or_insert(points.len());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point2;
    use crate::scenegraph::curve::{Curve2D, Polyline2D};

    fn profile() -> Curve2DRef {
        Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 2.0),
        ]))
        .into_ref()
    }

    #[test]
    fn a_revolution_defers_its_density_by_default() {
        assert_eq!(Revolution::from_profile(profile()).slices, None);
        assert_eq!(Revolution::new(profile(), 24).slices, Some(24));
    }

    #[test]
    fn a_revolution_profile_must_stay_off_the_far_side_of_the_axis() {
        assert!(Revolution::from_profile(profile()).is_valid(16).is_ok());

        let crossing = Curve2D::from(Polyline2D::new(vec![
            Point2::new(-1.0, 0.0),
            Point2::new(1.0, 2.0),
        ]))
        .into_ref();
        assert!(matches!(
            Revolution::from_profile(crossing).is_valid(16),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn swung_defaults_match_upstream() {
        assert_eq!(Swung::DEFAULT_DEGREE, 3);
        assert_eq!(Swung::DEFAULT_STRIDE, 0);
        // A default-constructed Swung is counter-clockwise, so its sweep
        // matches the winding every other primitive discretises to.
        assert!(Swung::linear(Vec::new(), Vec::new()).ccw);
    }

    #[test]
    fn swung_needs_one_angle_per_profile_in_ascending_order() {
        let ok = Swung::linear(vec![profile(), profile()], vec![0.0, 3.0]);
        assert!(ok.is_valid(16).is_ok());

        let mismatched = Swung::linear(vec![profile(), profile()], vec![0.0]);
        assert!(matches!(
            mismatched.is_valid(16),
            Err(Error::InvalidIndex(_))
        ));

        let unordered = Swung::linear(vec![profile(), profile()], vec![3.0, 0.0]);
        assert!(matches!(
            unordered.is_valid(16),
            Err(Error::DegenerateGeometry(_))
        ));

        let empty = Swung::linear(vec![], vec![]);
        assert!(matches!(empty.is_valid(16), Err(Error::DegenerateGeometry(_))));
    }
}
