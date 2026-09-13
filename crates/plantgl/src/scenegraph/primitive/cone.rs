//! The cone and the primitives upstream derives from it: cylinder, frustum and
//! paraboloid.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{cone,cylinder,
//! frustum,paraboloid}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream makes `Cylinder`, `Frustum` and `Paraboloid` subclasses of `Cone`
//! purely to inherit the `Radius`/`Height`/`Solid` fields — they have no
//! behaviour in common beyond that, and each has its own discretisation. The
//! port repeats the three fields rather than modelling an inheritance chain
//! that buys nothing.
//!
//! All four stand on the origin and grow along **+z**: the base ring sits at
//! `z = 0` and the apex or top ring at `z = height`.

use crate::error::{Error, Result};
use crate::math::Real;

use super::{is_positive, DEFAULT_SLICES};

/// `Disc::DEFAULT_RADIUS`, which upstream reuses as the default radius of
/// every surface of revolution.
pub(crate) const DEFAULT_RADIUS: Real = 0.5;

/// `Cone::DEFAULT_HEIGHT`.
pub(crate) const DEFAULT_HEIGHT: Real = 1.0;

/// `Cone::DEFAULT_SOLID` — closed with end caps.
pub(crate) const DEFAULT_SOLID: bool = true;

/// Upstream's `Cone` — a circular base at `z = 0` tapering to a point at
/// `z = height`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cone {
    /// The `Radius` field.
    pub radius: Real,
    /// The `Height` field.
    pub height: Real,
    /// The `Solid` field — whether the base is capped.
    pub solid: bool,
    /// The `Slices` field, or `None` to take the discretisation context's.
    pub slices: Option<u8>,
}

impl Cone {
    pub const DEFAULT_RADIUS: Real = DEFAULT_RADIUS;
    pub const DEFAULT_HEIGHT: Real = DEFAULT_HEIGHT;
    pub const DEFAULT_SOLID: bool = DEFAULT_SOLID;
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;

    /// `Cone(radius, height, solid, slices)`.
    pub fn new(radius: Real, height: Real, solid: bool, slices: u8) -> Self {
        Self {
            radius,
            height,
            solid,
            slices: Some(slices),
        }
    }

    /// A cone whose density the discretisation context decides.
    pub fn sized(radius: Real, height: Real) -> Self {
        Self {
            radius,
            height,
            solid: DEFAULT_SOLID,
            slices: None,
        }
    }

    /// Leaves the slice count to the discretisation context.
    pub fn with_context_slices(mut self) -> Self {
        self.slices = None;
        self
    }

    pub fn with_solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    /// `isValid()`.
    pub fn is_valid(&self) -> Result<()> {
        validate_radius_and_height("cone", self.radius, self.height)
    }
}

impl Default for Cone {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            height: DEFAULT_HEIGHT,
            solid: DEFAULT_SOLID,
            slices: None,
        }
    }
}

/// Upstream's `Cylinder` — a constant-radius tube from `z = 0` to
/// `z = height`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cylinder {
    pub radius: Real,
    pub height: Real,
    /// Whether both end caps are present.
    pub solid: bool,
    pub slices: Option<u8>,
}

impl Cylinder {
    pub const DEFAULT_RADIUS: Real = DEFAULT_RADIUS;
    pub const DEFAULT_HEIGHT: Real = DEFAULT_HEIGHT;
    pub const DEFAULT_SOLID: bool = DEFAULT_SOLID;
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;

    /// `Cylinder(radius, height, solid, slices)`.
    pub fn new(radius: Real, height: Real, solid: bool, slices: u8) -> Self {
        Self {
            radius,
            height,
            solid,
            slices: Some(slices),
        }
    }

    /// A cylinder whose density the discretisation context decides.
    pub fn sized(radius: Real, height: Real) -> Self {
        Self {
            radius,
            height,
            solid: DEFAULT_SOLID,
            slices: None,
        }
    }

    pub fn with_context_slices(mut self) -> Self {
        self.slices = None;
        self
    }

    pub fn with_solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    pub fn is_valid(&self) -> Result<()> {
        validate_radius_and_height("cylinder", self.radius, self.height)
    }
}

impl Default for Cylinder {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            height: DEFAULT_HEIGHT,
            solid: DEFAULT_SOLID,
            slices: None,
        }
    }
}

/// Upstream's `Frustum` — a truncated cone whose top ring has radius
/// `radius * taper`.
///
/// This is the stem workhorse: a `taper` of 1 is a cylinder and a taper of 0 a
/// cone, so one primitive covers every internode. Note that `Frustum`'s taper
/// is a **field of the primitive**, applied while the rings are generated —
/// quite distinct from the `Tapered` *transformation*, which deforms a
/// discretised point set after the fact. See
/// [`crate::scenegraph::transform`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frustum {
    /// The radius of the base ring, at `z = 0`.
    pub radius: Real,
    pub height: Real,
    /// The `Taper` field — the top ring's radius as a fraction of `radius`.
    pub taper: Real,
    pub solid: bool,
    pub slices: Option<u8>,
}

impl Frustum {
    pub const DEFAULT_RADIUS: Real = DEFAULT_RADIUS;
    pub const DEFAULT_HEIGHT: Real = DEFAULT_HEIGHT;
    pub const DEFAULT_SOLID: bool = DEFAULT_SOLID;
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;
    /// `Frustum::DEFAULT_TAPER`.
    pub const DEFAULT_TAPER: Real = 0.5;

    /// `Frustum(radius, height, taper, solid, slices)`.
    pub fn new(radius: Real, height: Real, taper: Real, solid: bool, slices: u8) -> Self {
        Self {
            radius,
            height,
            taper,
            solid,
            slices: Some(slices),
        }
    }

    /// A frustum whose density the discretisation context decides.
    pub fn sized(radius: Real, height: Real, taper: Real) -> Self {
        Self {
            radius,
            height,
            taper,
            solid: DEFAULT_SOLID,
            slices: None,
        }
    }

    pub fn with_context_slices(mut self) -> Self {
        self.slices = None;
        self
    }

    pub fn with_solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    /// The radius of the top ring, at `z = height`.
    pub fn top_radius(&self) -> Real {
        self.radius * self.taper
    }

    /// `isValid()` — upstream additionally requires a non-negative taper.
    pub fn is_valid(&self) -> Result<()> {
        validate_radius_and_height("frustum", self.radius, self.height)?;
        if !self.taper.is_finite() || self.taper < 0.0 {
            return Err(Error::degenerate(format!(
                "frustum taper must be non-negative, got {}",
                self.taper
            )));
        }
        Ok(())
    }
}

impl Default for Frustum {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            height: DEFAULT_HEIGHT,
            taper: Self::DEFAULT_TAPER,
            solid: DEFAULT_SOLID,
            slices: None,
        }
    }
}

/// Upstream's `Paraboloid` — a surface of revolution whose profile is
/// `z = height * (1 - (r / radius)^shape)`.
///
/// `shape` of 2 is the true paraboloid; larger values flatten the top and
/// smaller ones sharpen it, which is how upstream shapes fruit and bud
/// envelopes.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Paraboloid {
    pub radius: Real,
    pub height: Real,
    /// The `Shape` field — the exponent in the profile.
    pub shape: Real,
    pub solid: bool,
    pub slices: Option<u8>,
    /// The `Stacks` field — subdivisions along the profile, or `None` to take
    /// the discretisation context's.
    pub stacks: Option<u8>,
}

impl Paraboloid {
    pub const DEFAULT_RADIUS: Real = DEFAULT_RADIUS;
    pub const DEFAULT_HEIGHT: Real = DEFAULT_HEIGHT;
    pub const DEFAULT_SOLID: bool = DEFAULT_SOLID;
    pub const DEFAULT_SLICES: u8 = DEFAULT_SLICES;
    /// `Paraboloid::DEFAULT_SHAPE`.
    pub const DEFAULT_SHAPE: Real = 2.0;
    /// `Paraboloid::DEFAULT_STACKS`.
    pub const DEFAULT_STACKS: u8 = 8;

    /// `Paraboloid(radius, height, shape, solid, slices, stacks)`.
    pub fn new(
        radius: Real,
        height: Real,
        shape: Real,
        solid: bool,
        slices: u8,
        stacks: u8,
    ) -> Self {
        Self {
            radius,
            height,
            shape,
            solid,
            slices: Some(slices),
            stacks: Some(stacks),
        }
    }

    /// A paraboloid whose density the discretisation context decides.
    pub fn sized(radius: Real, height: Real, shape: Real) -> Self {
        Self {
            radius,
            height,
            shape,
            solid: DEFAULT_SOLID,
            slices: None,
            stacks: None,
        }
    }

    pub fn with_context_density(mut self) -> Self {
        self.slices = None;
        self.stacks = None;
        self
    }

    pub fn with_solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    /// `isValid()` — upstream additionally requires a positive shape.
    pub fn is_valid(&self) -> Result<()> {
        validate_radius_and_height("paraboloid", self.radius, self.height)?;
        if !is_positive(self.shape) {
            return Err(Error::degenerate(format!(
                "paraboloid shape must be positive, got {}",
                self.shape
            )));
        }
        Ok(())
    }
}

impl Default for Paraboloid {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            height: DEFAULT_HEIGHT,
            shape: Self::DEFAULT_SHAPE,
            solid: DEFAULT_SOLID,
            slices: None,
            stacks: None,
        }
    }
}

fn validate_radius_and_height(what: &str, radius: Real, height: Real) -> Result<()> {
    if !is_positive(radius) {
        return Err(Error::degenerate(format!(
            "{what} radius must be positive, got {radius}"
        )));
    }
    if !is_positive(height) {
        return Err(Error::degenerate(format!(
            "{what} height must be positive, got {height}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn defaults_match_upstreams_constants() {
        assert_relative_eq!(Cone::default().radius, 0.5);
        assert_relative_eq!(Cone::default().height, 1.0);
        assert!(Cone::default().solid);
        assert_relative_eq!(Frustum::default().taper, 0.5);
        assert_relative_eq!(Paraboloid::default().shape, 2.0);
    }

    #[test]
    fn default_construction_defers_density_to_the_context() {
        assert_eq!(Cone::default().slices, None);
        assert_eq!(Cylinder::default().slices, None);
        assert_eq!(Frustum::default().slices, None);
        assert_eq!(Paraboloid::default().stacks, None);
    }

    #[test]
    fn upstreams_constructor_signature_pins_the_density() {
        assert_eq!(Cylinder::new(1.0, 2.0, true, 32).slices, Some(32));
        assert_eq!(Paraboloid::new(1.0, 2.0, 2.0, true, 32, 16).stacks, Some(16));
        assert_eq!(Cylinder::new(1.0, 2.0, true, 32).with_context_slices().slices, None);
    }

    #[test]
    fn frustum_taper_scales_the_top_ring() {
        assert_relative_eq!(Frustum::new(2.0, 1.0, 0.25, true, 8).top_radius(), 0.5);
    }

    #[test]
    fn validity_rejects_non_positive_dimensions() {
        assert!(Cone::default().is_valid().is_ok());
        assert!(Cone::sized(0.0, 1.0).is_valid().is_err());
        assert!(Cylinder::sized(1.0, -1.0).is_valid().is_err());
        assert!(Frustum::sized(1.0, 1.0, -0.5).is_valid().is_err());
        assert!(Frustum::sized(1.0, 1.0, 0.0).is_valid().is_ok());
        assert!(Paraboloid::sized(1.0, 1.0, 0.0).is_valid().is_err());
    }
}
