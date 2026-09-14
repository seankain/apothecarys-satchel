//! Vector, matrix and scalar helpers.
//!
//! Ported from PlantGL `src/cpp/plantgl/math/{util_vector,util_matrix,util_math}.h`
//! and `src/cpp/plantgl/tool/util_types.h` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream hand-rolls `Vector2`/`Vector3`/`Matrix3`/`Matrix4`. We alias
//! `nalgebra` instead — it is already in the dependency graph via the engine,
//! so geometry crosses the engine boundary without conversion — and keep only
//! the constants and predicates that upstream's algorithms actually depend on.

pub mod frame;

pub use frame::Frame;

/// Upstream's `real_t`. PlantGL declares `typedef float real_t` unless
/// `PGL_USE_DOUBLE` is set, so `f32` is the faithful default.
pub type Real = f32;

pub type Vec2 = nalgebra::Vector2<Real>;
pub type Vec3 = nalgebra::Vector3<Real>;
pub type Vec4 = nalgebra::Vector4<Real>;
pub type Point2 = nalgebra::Point2<Real>;
pub type Point3 = nalgebra::Point3<Real>;
pub type Mat3 = nalgebra::Matrix3<Real>;
pub type Mat4 = nalgebra::Matrix4<Real>;
pub type UnitVec3 = nalgebra::Unit<Vec3>;

/// `GEOM_EPSILON` — upstream's working tolerance for geometric comparisons
/// (`tool/util_types.h`).
pub const EPSILON: Real = 1e-5;

/// `GEOM_TOLERANCE` — upstream's tighter tolerance, used where a comparison
/// must not absorb genuine detail (`tool/util_types.h`).
pub const TOLERANCE: Real = 1e-10;

/// `GEOM_RAD` — one degree in radians.
pub const RAD: Real = 0.017_453_292;

/// `GEOM_DEG` — one radian in degrees.
pub const DEG: Real = 57.295_78;

/// Degrees to radians.
#[inline]
pub fn to_radians(degrees: Real) -> Real {
    degrees * RAD
}

/// Radians to degrees.
#[inline]
pub fn to_degrees(radians: Real) -> Real {
    radians * DEG
}

/// Equality within [`EPSILON`], upstream's default comparison for reals.
#[inline]
pub fn approx_eq(a: Real, b: Real) -> bool {
    (a - b).abs() < EPSILON
}

/// Equality within an explicit tolerance.
#[inline]
pub fn approx_eq_tol(a: Real, b: Real, tolerance: Real) -> bool {
    (a - b).abs() < tolerance
}

/// Component-wise [`approx_eq`] over two vectors.
#[inline]
pub fn vec3_approx_eq(a: &Vec3, b: &Vec3) -> bool {
    approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z)
}

/// Whether every component is finite. Upstream's `isValid()` on vectors.
#[inline]
pub fn vec3_is_valid(v: &Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

/// The signed angle from `v1` to `v2`, as upstream's
/// `angle(const Vector2&, const Vector2&)` — `atan2(cross, dot)`, so it
/// carries the sense of the turn and not just its size.
///
/// This is what a 2D guide measures a curve's deviation with.
#[inline]
pub fn angle2(v1: &Vec2, v2: &Vec2) -> Real {
    let cross = v1.x * v2.y - v1.y * v2.x;
    cross.atan2(v1.dot(v2))
}

/// The unsigned angle between two vectors, as upstream's
/// `angle(const Vector3&, const Vector3&)`.
///
/// `atan2(|v1 × v2|, v1 · v2)` rather than `acos` of the normalised dot: it
/// stays accurate for nearly parallel and nearly opposed inputs, where the
/// cosine is flat.
#[inline]
pub fn angle3(v1: &Vec3, v2: &Vec3) -> Real {
    let cross = v1.cross(v2);
    cross.norm().atan2(v1.dot(v2))
}

/// The angle between two vectors, signed about `axis` — upstream's
/// `angle(const Vector3&, const Vector3&, const Vector3&)`.
#[inline]
pub fn angle3_about(v1: &Vec3, v2: &Vec3, axis: &Vec3) -> Real {
    let cross = v1.cross(v2);
    let sinus = cross.norm();
    let sinus = if cross.dot(axis) < 0.0 { -sinus } else { sinus };
    sinus.atan2(v1.dot(v2))
}

/// Euler rotation about Z, then Y, then X, as upstream's
/// `Matrix3::eulerRotationZYX(Vector3(azimuth, elevation, roll))`.
///
/// This is the rotation `EulerRotated` applies, and it is *not* the convention
/// `nalgebra::Rotation3::from_euler_angles` uses, so it is written out here
/// rather than delegated.
pub fn euler_rotation_zyx(azimuth: Real, elevation: Real, roll: Real) -> Mat3 {
    let (sz, cz) = roll.sin_cos();
    let (sy, cy) = elevation.sin_cos();
    let (sx, cx) = azimuth.sin_cos();
    let cxsy = cx * sy;
    let sxsy = sx * sy;

    Mat3::new(
        cx * cy,
        cxsy * sz - sx * cz,
        cxsy * cz + sx * sz,
        sx * cy,
        cx * cz + sxsy * sz,
        sxsy * cz - cx * sz,
        -sy,
        cy * sz,
        cy * cz,
    )
}

/// The 3×3 basis whose columns are `primary`, `secondary` and their cross
/// product, as upstream's `BaseOrientation` builds for `Oriented`.
///
/// `secondary` is re-orthogonalised against `primary` first; upstream requires
/// the caller to supply orthogonal vectors and asserts on it, and silently
/// producing a skewed basis is worse than correcting one.
pub fn orthonormal_basis(primary: &Vec3, secondary: &Vec3) -> Option<Mat3> {
    let x = primary.try_normalize(TOLERANCE)?;
    let projected = secondary - x * x.dot(secondary);
    let y = projected.try_normalize(TOLERANCE)?;
    let z = x.cross(&y);
    Some(Mat3::from_columns(&[x, y, z]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn degrees_and_radians_round_trip() {
        for degrees in [0.0, 1.0, 45.0, 90.0, 180.0, 359.0] {
            assert!(approx_eq(to_degrees(to_radians(degrees)), degrees));
        }
    }

    #[test]
    fn euler_zyx_is_a_rotation() {
        let m = euler_rotation_zyx(0.3, -0.7, 1.1);
        assert_relative_eq!(m.determinant(), 1.0, epsilon = 1e-5);
        let should_be_identity = m * m.transpose();
        assert_relative_eq!(should_be_identity, Mat3::identity(), epsilon = 1e-5);
    }

    #[test]
    fn euler_zyx_azimuth_only_rotates_about_z() {
        // Upstream's azimuth is the x component of eulerRotationZYX, which
        // ends up as a rotation in the xy plane.
        let m = euler_rotation_zyx(std::f32::consts::FRAC_PI_2, 0.0, 0.0);
        let rotated = m * Vec3::x();
        assert_relative_eq!(rotated, Vec3::y(), epsilon = 1e-5);
    }

    #[test]
    fn orthonormal_basis_orthonormalises_a_skewed_secondary() {
        let basis = orthonormal_basis(&Vec3::x(), &Vec3::new(0.5, 1.0, 0.0)).unwrap();
        assert_relative_eq!(basis.column(0).into_owned(), Vec3::x(), epsilon = 1e-6);
        assert_relative_eq!(basis.column(1).into_owned(), Vec3::y(), epsilon = 1e-6);
        assert_relative_eq!(basis.column(2).into_owned(), Vec3::z(), epsilon = 1e-6);
    }

    #[test]
    fn orthonormal_basis_rejects_parallel_inputs() {
        assert!(orthonormal_basis(&Vec3::x(), &Vec3::new(2.0, 0.0, 0.0)).is_none());
        assert!(orthonormal_basis(&Vec3::zeros(), &Vec3::y()).is_none());
    }
}
