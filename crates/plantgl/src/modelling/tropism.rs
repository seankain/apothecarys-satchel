//! Bending a turtle's frame towards a direction — tropism and reflections.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/turtle.cpp`
//! (`Turtle::_tendTo`, `Turtle::_applyTropism`, `Turtle::leftReflection`,
//! `Turtle::upReflection`, `Turtle::headingReflection`)
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # What tropism is
//!
//! ABOP §2.3's "tend to" operator, which is upstream's `_tendTo`: rotate the
//! whole frame about `heading × target` by `strength` times the angle between
//! the heading and the target. Applied on every forward move with
//! `target = tropism` and `strength = elasticity`, it is the difference
//! between a plant that grows in whatever direction its L-system happened to
//! point it and one that knows which way is down.
//!
//! `tropism = -Y` in a Y-up world is gravitropism — drooping vines, laden
//! stems. A tropism aimed at a light source is phototropism — the shade plant
//! that leans out of the shade. Both are two numbers on the state, not a new
//! production rule.

use crate::math::{Mat3, Real, Vec3, EPSILON};

/// `Turtle::_tendTo(t, strength)` — the rotation that bends `heading` towards
/// `target`, or `None` when there is nothing to do.
///
/// The angle is `atan2(|h × t|, h · t)`, which is the full turn towards the
/// target rather than its sine, so `strength = 1` lands exactly on the target
/// however far away it started. `strength` is the elasticity.
///
/// Two cases upstream treats separately and so does this:
///
/// - The cross product is usable: rotate about it.
/// - The cross product has collapsed and the heading points *away* from the
///   target: there is no preferred axis, so upstream turns about `up` by
///   `π * strength`. Any axis normal to the heading would do; `up` is the one
///   that keeps a planar plant planar.
///
/// A heading that already points at the target returns `None`.
pub fn tend_to(heading: &Vec3, up: &Vec3, target: &Vec3, strength: Real) -> Option<Mat3> {
    let cross = heading.cross(target);
    let sinus = cross.norm();
    if sinus > EPSILON {
        let axis = cross / sinus;
        let cosinus = heading.dot(target);
        let angle = sinus.atan2(cosinus);
        return axis_rotation(&axis, angle * strength);
    }

    // Collinear. Only the anti-parallel case needs a turn, and upstream's
    // test is `cos < GEOM_EPSILON` rather than `cos < 0` — a heading exactly
    // perpendicular cannot reach here, so the two agree.
    let cosinus = heading.dot(target);
    if cosinus < EPSILON {
        return axis_rotation(up, std::f32::consts::PI * strength);
    }
    None
}

/// `Matrix3::axisRotation`, returning `None` for an axis that cannot be
/// normalised or an angle that would do nothing.
pub fn axis_rotation(axis: &Vec3, angle: Real) -> Option<Mat3> {
    if angle.abs() < Real::EPSILON {
        return None;
    }
    let axis = nalgebra::Unit::try_new(*axis, 1e-10)?;
    Some(nalgebra::Rotation3::from_axis_angle(&axis, angle).into_inner())
}

/// Which sense a rotation family runs in — upstream's `TurtleParam::reflection`
/// triple, one `±1` per family.
///
/// `headingReflection` flips `rollL`, `leftReflection` flips `left`/`right`,
/// and `upReflection` flips `up`/`down`. Mirroring a plant is a sign change
/// here rather than a second set of productions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reflection {
    /// `reflection.x` — the roll family.
    Heading,
    /// `reflection.y` — the `left`/`right` family.
    Left,
    /// `reflection.z` — the `up`/`down` family.
    Up,
}

impl Reflection {
    /// The component of the reflection triple this family reads.
    pub fn component(self, reflection: &Vec3) -> Real {
        match self {
            Reflection::Heading => reflection.x,
            Reflection::Left => reflection.y,
            Reflection::Up => reflection.z,
        }
    }

    /// Flips this family's sign in place.
    pub fn flip(self, reflection: &mut Vec3) {
        match self {
            Reflection::Heading => reflection.x = -reflection.x,
            Reflection::Left => reflection.y = -reflection.y,
            Reflection::Up => reflection.z = -reflection.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn full_strength_lands_on_the_target() {
        let heading = Vec3::z();
        let target = Vec3::new(0.0, 1.0, 1.0).normalize();
        let m = tend_to(&heading, &Vec3::x(), &target, 1.0).unwrap();
        assert_relative_eq!(m * heading, target, epsilon = 1e-6);
    }

    #[test]
    fn half_strength_covers_half_the_angle() {
        let heading = Vec3::z();
        let target = Vec3::y();
        let m = tend_to(&heading, &Vec3::x(), &target, 0.5).unwrap();
        let bent = m * heading;
        let angle = bent.dot(&heading).acos();
        assert_relative_eq!(angle, std::f32::consts::FRAC_PI_4, epsilon = 1e-5);
    }

    #[test]
    fn an_aligned_heading_does_not_move() {
        assert!(tend_to(&Vec3::z(), &Vec3::x(), &Vec3::z(), 1.0).is_none());
    }

    #[test]
    fn an_opposed_heading_turns_about_up() {
        let m = tend_to(&Vec3::z(), &Vec3::x(), &(-Vec3::z()), 1.0).unwrap();
        assert_relative_eq!(m * Vec3::z(), -Vec3::z(), epsilon = 1e-6);
    }

    #[test]
    fn reflections_flip_one_family_each() {
        let mut reflection = Vec3::new(1.0, 1.0, 1.0);
        Reflection::Left.flip(&mut reflection);
        assert_eq!(reflection, Vec3::new(1.0, -1.0, 1.0));
        assert_eq!(Reflection::Left.component(&reflection), -1.0);
        assert_eq!(Reflection::Up.component(&reflection), 1.0);
        Reflection::Left.flip(&mut reflection);
        assert_eq!(reflection, Vec3::new(1.0, 1.0, 1.0));
    }
}
