//! Tensor-product Bézier and NURBS surfaces.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{bezierpatch,
//! nurbspatch}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # One control-net layout, because upstream has two
//!
//! Upstream stores both patches in a `Point4Matrix` and reads it with
//! `getAt(row, column)`, but the two classes disagree about which axis is
//! which. `BezierPatch::getPointAt` runs de Casteljau along `getRow(j)` for
//! `j <= vDegree` and defines `getUDegree() = getRowSize() - 1`, so its rows
//! are indexed by **v** and each row runs along u. `NurbsPatch::getPointAt`
//! reads `getAt(uspan - p + k, vspan - q + l)` and sizes its u knot vector from
//! `getColumnSize()`, so its rows are indexed by **u** — the transpose. The
//! two are individually self-consistent and mutually contradictory, and a
//! control net moved from one class to the other in upstream comes out
//! transposed.
//!
//! The port picks [`NurbsPatch`]'s reading for both: [`CtrlPointMatrix`] is
//! indexed `[u][v]`, `rows()` is the u count and `columns()` is the v count.
//! A net transcribed from an upstream `BezierPatch` therefore has to be
//! transposed on the way in; `tools/differential/upstream_measure.py` does
//! exactly that for its Bézier patch cases, and the differential harness is
//! what pins the claim that these two layouts are transposes.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point3, Real, Vec3, Vec4};

use super::spline::{
    basis_derivatives, basis_functions, de_casteljau, default_knots, find_span, hodograph, project,
    rational_derivatives, to_homogeneous_3d, validate_knots, widen, Homogeneous, DEFAULT_STRIDE,
};

/// A rectangular grid of weighted control points, indexed `[u][v]`.
///
/// Each point is `(x, y, z, weight)`, the layout
/// [`super::spline`] documents.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtrlPointMatrix {
    points: Vec<Vec4>,
    u_count: usize,
    v_count: usize,
}

impl CtrlPointMatrix {
    /// Builds from rows of constant `u`, each row running along `v`.
    ///
    /// Every row must be the same length; a ragged net is
    /// [`Error::InvalidIndex`], because upstream's `Array2` cannot represent
    /// one and a caller that produced one has a bug upstream of here.
    pub fn from_rows(rows: Vec<Vec<Vec4>>) -> Result<Self> {
        let u_count = rows.len();
        let v_count = rows.first().map_or(0, Vec::len);
        if let Some((i, row)) = rows.iter().enumerate().find(|(_, r)| r.len() != v_count) {
            return Err(Error::invalid_index(format!(
                "control net row {i} has {} points, the first had {v_count}",
                row.len()
            )));
        }
        Ok(Self {
            points: rows.into_iter().flatten().collect(),
            u_count,
            v_count,
        })
    }

    /// Builds from rows of Cartesian points, every weight 1.
    pub fn from_point_rows(rows: Vec<Vec<Point3>>) -> Result<Self> {
        Self::from_rows(
            rows.into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|p| Vec4::new(p.x, p.y, p.z, 1.0))
                        .collect()
                })
                .collect(),
        )
    }

    /// The number of control points along `u`.
    pub fn rows(&self) -> usize {
        self.u_count
    }

    /// The number of control points along `v`.
    pub fn columns(&self) -> usize {
        self.v_count
    }

    pub fn get(&self, u: usize, v: usize) -> Option<Vec4> {
        if u >= self.u_count || v >= self.v_count {
            return None;
        }
        self.points.get(u * self.v_count + v).copied()
    }

    pub fn points(&self) -> &[Vec4] {
        &self.points
    }

    fn homogeneous(&self) -> Vec<Vec<Homogeneous>> {
        (0..self.u_count)
            .map(|u| {
                (0..self.v_count)
                    .map(|v| to_homogeneous_3d(&self.points[u * self.v_count + v]))
                    .collect()
            })
            .collect()
    }

    fn is_valid(&self, what: &str) -> Result<()> {
        if self.u_count < 2 || self.v_count < 2 {
            return Err(Error::degenerate(format!(
                "{what} needs at least a 2x2 control net, got {}x{}",
                self.u_count, self.v_count
            )));
        }
        for (i, point) in self.points.iter().enumerate() {
            if !point.w.is_finite() || point.w.abs() < 1e-10 {
                return Err(Error::degenerate(format!(
                    "{what} control point {i} has weight {}, which must be non-zero",
                    point.w
                )));
            }
        }
        Ok(())
    }
}

/// Upstream's `BezierPatch` — a tensor-product rational Bézier surface over
/// `[0, 1]²`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierPatch {
    pub ctrl_points: Arc<CtrlPointMatrix>,
    /// `UStride`/`VStride` — the sample counts the discretizer uses. `None`
    /// defers to [`DiscretizeCtx`](crate::algo::discretize::DiscretizeCtx).
    pub u_stride: Option<u32>,
    pub v_stride: Option<u32>,
    pub ccw: bool,
}

impl BezierPatch {
    /// `BezierPatch::DEFAULT_STRIDE`.
    pub const DEFAULT_STRIDE: u32 = DEFAULT_STRIDE;

    pub fn new(ctrl_points: CtrlPointMatrix) -> Self {
        Self {
            ctrl_points: Arc::new(ctrl_points),
            u_stride: None,
            v_stride: None,
            ccw: true,
        }
    }

    pub fn with_strides(mut self, u_stride: u32, v_stride: u32) -> Self {
        self.u_stride = Some(u_stride);
        self.v_stride = Some(v_stride);
        self
    }

    /// `getUDegree()` — one less than the number of control points along `u`.
    pub fn u_degree(&self) -> usize {
        self.ctrl_points.rows().saturating_sub(1)
    }

    /// `getVDegree()`.
    pub fn v_degree(&self) -> usize {
        self.ctrl_points.columns().saturating_sub(1)
    }

    pub fn is_valid(&self) -> Result<()> {
        self.ctrl_points.is_valid("a Bézier patch")
    }

    /// `getPointAt(u, v)`.
    pub fn eval(&self, u: Real, v: Real) -> Result<Point3> {
        self.is_valid()?;
        let grid = self.ctrl_points.homogeneous();
        Ok(narrow(project(bezier_surface_point(
            &grid,
            clamp_unit(u),
            clamp_unit(v),
        ))))
    }

    /// The partial derivative along `u`, by the quotient rule.
    pub fn u_tangent(&self, u: Real, v: Real) -> Result<Vec3> {
        self.is_valid()?;
        let grid = self.ctrl_points.homogeneous();
        let (u, v) = (clamp_unit(u), clamp_unit(v));
        let point = bezier_surface_point(&grid, u, v);
        let derivative = bezier_surface_du(&grid, u, v);
        Ok(narrow_vec(rational_derivatives(&[point, derivative])[1]))
    }

    /// The partial derivative along `v`.
    pub fn v_tangent(&self, u: Real, v: Real) -> Result<Vec3> {
        self.is_valid()?;
        let grid = self.ctrl_points.homogeneous();
        let (u, v) = (clamp_unit(u), clamp_unit(v));
        let point = bezier_surface_point(&grid, u, v);
        let derivative = bezier_surface_dv(&grid, u, v);
        Ok(narrow_vec(rational_derivatives(&[point, derivative])[1]))
    }

    /// `getNormalAt(u, v)` — the normalised cross product of the two partials.
    pub fn normal(&self, u: Real, v: Real) -> Result<Vec3> {
        surface_normal(self.u_tangent(u, v)?, self.v_tangent(u, v)?)
    }
}

/// Upstream's `NurbsPatch` — a tensor-product rational B-spline surface.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NurbsPatch {
    pub ctrl_points: Arc<CtrlPointMatrix>,
    pub u_degree: usize,
    pub v_degree: usize,
    pub u_knots: Arc<Vec<Real>>,
    pub v_knots: Arc<Vec<Real>>,
    pub u_stride: Option<u32>,
    pub v_stride: Option<u32>,
    pub ccw: bool,
}

impl NurbsPatch {
    /// A patch with the default clamped uniform knot vectors, as upstream's
    /// builder produces when no knot list is given.
    pub fn new(ctrl_points: CtrlPointMatrix, u_degree: usize, v_degree: usize) -> Self {
        let u_degree = u_degree.min(ctrl_points.rows().saturating_sub(1)).max(1);
        let v_degree = v_degree.min(ctrl_points.columns().saturating_sub(1)).max(1);
        let u_knots = default_knots(ctrl_points.rows(), u_degree);
        let v_knots = default_knots(ctrl_points.columns(), v_degree);
        Self {
            ctrl_points: Arc::new(ctrl_points),
            u_degree,
            v_degree,
            u_knots: Arc::new(u_knots),
            v_knots: Arc::new(v_knots),
            u_stride: None,
            v_stride: None,
            ccw: true,
        }
    }

    /// A patch over explicit knot vectors, validated by
    /// [`super::spline::validate_knots`].
    pub fn with_knots(
        ctrl_points: CtrlPointMatrix,
        u_degree: usize,
        v_degree: usize,
        u_knots: Vec<Real>,
        v_knots: Vec<Real>,
    ) -> Result<Self> {
        ctrl_points.is_valid("a NURBS patch")?;
        validate_knots(&u_knots, ctrl_points.rows(), u_degree)?;
        validate_knots(&v_knots, ctrl_points.columns(), v_degree)?;
        Ok(Self {
            ctrl_points: Arc::new(ctrl_points),
            u_degree,
            v_degree,
            u_knots: Arc::new(u_knots),
            v_knots: Arc::new(v_knots),
            u_stride: None,
            v_stride: None,
            ccw: true,
        })
    }

    pub fn with_strides(mut self, u_stride: u32, v_stride: u32) -> Self {
        self.u_stride = Some(u_stride);
        self.v_stride = Some(v_stride);
        self
    }

    /// `getFirstUKnot()`.
    pub fn first_u_knot(&self) -> Real {
        self.u_knots[0]
    }

    /// `getLastUKnot()`.
    pub fn last_u_knot(&self) -> Real {
        self.u_knots[self.u_knots.len() - 1]
    }

    /// `getFirstVKnot()`.
    pub fn first_v_knot(&self) -> Real {
        self.v_knots[0]
    }

    /// `getLastVKnot()`.
    pub fn last_v_knot(&self) -> Real {
        self.v_knots[self.v_knots.len() - 1]
    }

    pub fn is_valid(&self) -> Result<()> {
        self.ctrl_points.is_valid("a NURBS patch")?;
        validate_knots(&self.u_knots, self.ctrl_points.rows(), self.u_degree)?;
        validate_knots(&self.v_knots, self.ctrl_points.columns(), self.v_degree)
    }

    /// `getPointAt(u, v)`.
    pub fn eval(&self, u: Real, v: Real) -> Result<Point3> {
        Ok(narrow(project(self.evaluate(u, v, 0, 0)?)))
    }

    /// `getUTangentAt(u, v)` — `getDerivativeAt(u, v, 1, 0)`.
    pub fn u_tangent(&self, u: Real, v: Real) -> Result<Vec3> {
        let point = self.evaluate(u, v, 0, 0)?;
        let derivative = self.evaluate(u, v, 1, 0)?;
        Ok(narrow_vec(rational_derivatives(&[point, derivative])[1]))
    }

    /// `getVTangentAt(u, v)`.
    pub fn v_tangent(&self, u: Real, v: Real) -> Result<Vec3> {
        let point = self.evaluate(u, v, 0, 0)?;
        let derivative = self.evaluate(u, v, 0, 1)?;
        Ok(narrow_vec(rational_derivatives(&[point, derivative])[1]))
    }

    /// `getNormalAt(u, v)`.
    pub fn normal(&self, u: Real, v: Real) -> Result<Vec3> {
        surface_normal(self.u_tangent(u, v)?, self.v_tangent(u, v)?)
    }

    /// The `(du, dv)` partial of the *homogeneous* surface.
    fn evaluate(&self, u: Real, v: Real, du: usize, dv: usize) -> Result<Homogeneous> {
        self.is_valid()?;
        let u_knots = widen(&self.u_knots);
        let v_knots = widen(&self.v_knots);
        let u = (u as f64).clamp(u_knots[0], u_knots[u_knots.len() - 1]);
        let v = (v as f64).clamp(v_knots[0], v_knots[v_knots.len() - 1]);

        if du > self.u_degree || dv > self.v_degree {
            return Ok(Homogeneous::zeros());
        }

        let u_span = find_span(u, self.u_degree, &u_knots);
        let v_span = find_span(v, self.v_degree, &v_knots);
        let u_basis = if du == 0 {
            basis_functions(u_span, u, self.u_degree, &u_knots)
        } else {
            basis_derivatives(du, u, u_span, self.u_degree, &u_knots).remove(du)
        };
        let v_basis = if dv == 0 {
            basis_functions(v_span, v, self.v_degree, &v_knots)
        } else {
            basis_derivatives(dv, v, v_span, self.v_degree, &v_knots).remove(dv)
        };

        let grid = self.ctrl_points.homogeneous();
        let mut point = Homogeneous::zeros();
        for (l, v_weight) in v_basis.iter().enumerate() {
            let mut row = Homogeneous::zeros();
            for (k, u_weight) in u_basis.iter().enumerate() {
                row += grid[u_span - self.u_degree + k][v_span - self.v_degree + l] * *u_weight;
            }
            point += row * *v_weight;
        }
        Ok(point)
    }
}

// --- Shared surface evaluation ----------------------------------------------

/// de Casteljau in `u` down each column, then in `v` across the results.
///
/// Upstream picks whichever of the two orders costs fewer interpolations; the
/// answer is the same either way — a tensor-product surface is symmetric in the
/// order its two directions are reduced — so the port writes the one order.
fn bezier_surface_point(grid: &[Vec<Homogeneous>], u: f64, v: f64) -> Homogeneous {
    let along_v: Vec<Homogeneous> = (0..grid[0].len())
        .map(|j| {
            let column: Vec<Homogeneous> = grid.iter().map(|row| row[j]).collect();
            de_casteljau(&column, u)
        })
        .collect();
    de_casteljau(&along_v, v)
}

fn bezier_surface_du(grid: &[Vec<Homogeneous>], u: f64, v: f64) -> Homogeneous {
    if grid.len() < 2 {
        return Homogeneous::zeros();
    }
    let along_v: Vec<Homogeneous> = (0..grid[0].len())
        .map(|j| {
            let column: Vec<Homogeneous> = grid.iter().map(|row| row[j]).collect();
            de_casteljau(&hodograph(&column), u)
        })
        .collect();
    de_casteljau(&along_v, v)
}

fn bezier_surface_dv(grid: &[Vec<Homogeneous>], u: f64, v: f64) -> Homogeneous {
    if grid[0].len() < 2 {
        return Homogeneous::zeros();
    }
    let along_u: Vec<Homogeneous> = grid
        .iter()
        .map(|row| de_casteljau(&hodograph(row), v))
        .collect();
    de_casteljau(&along_u, u)
}

/// `cross(uTangent, vTangent)` normalised, with a degenerate cross product
/// reported rather than returned as a zero vector a renderer would shade with.
fn surface_normal(u_tangent: Vec3, v_tangent: Vec3) -> Result<Vec3> {
    u_tangent
        .cross(&v_tangent)
        .try_normalize(1e-12)
        .ok_or_else(|| {
            Error::degenerate("the surface has no normal here: its two tangents are parallel")
        })
}

fn narrow(v: nalgebra::Vector3<f64>) -> Point3 {
    Point3::new(v.x as Real, v.y as Real, v.z as Real)
}

fn narrow_vec(v: nalgebra::Vector3<f64>) -> Vec3 {
    Vec3::new(v.x as Real, v.y as Real, v.z as Real)
}

fn clamp_unit(u: Real) -> f64 {
    (u as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// A saddle: z = x·y over the unit square, which a bi-linear patch
    /// represents exactly.
    fn saddle() -> CtrlPointMatrix {
        CtrlPointMatrix::from_point_rows(vec![
            vec![Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            vec![Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0)],
        ])
        .unwrap()
    }

    fn bumped() -> CtrlPointMatrix {
        CtrlPointMatrix::from_point_rows(vec![
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.5),
                Point3::new(0.0, 2.0, 0.0),
            ],
            vec![
                Point3::new(1.0, 0.0, 0.4),
                Point3::new(1.0, 1.0, 1.6),
                Point3::new(1.0, 2.0, -0.3),
            ],
            vec![
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(2.0, 1.0, 0.2),
                Point3::new(2.0, 2.0, 0.0),
            ],
        ])
        .unwrap()
    }

    #[test]
    fn a_ragged_control_net_is_rejected() {
        let rows = vec![
            vec![Vec4::new(0.0, 0.0, 0.0, 1.0)],
            vec![Vec4::new(1.0, 0.0, 0.0, 1.0), Vec4::new(1.0, 1.0, 0.0, 1.0)],
        ];
        assert!(matches!(
            CtrlPointMatrix::from_rows(rows),
            Err(Error::InvalidIndex(_))
        ));
    }

    #[test]
    fn a_bilinear_patch_is_its_own_control_net() {
        let patch = BezierPatch::new(saddle());
        assert_relative_eq!(patch.eval(0.0, 0.0).unwrap(), Point3::new(0.0, 0.0, 0.0));
        assert_relative_eq!(patch.eval(1.0, 1.0).unwrap(), Point3::new(1.0, 1.0, 1.0));
        // z = u·v inside.
        for (u, v) in [(0.5, 0.5), (0.25, 0.75), (1.0, 0.5)] {
            let point = patch.eval(u, v).unwrap();
            assert_relative_eq!(point.z, u * v, epsilon = 1e-6);
        }
    }

    #[test]
    fn patch_corners_are_the_corner_control_points() {
        let patch = BezierPatch::new(bumped());
        let net = &patch.ctrl_points;
        for (u, v, corner) in [
            (0.0, 0.0, net.get(0, 0).unwrap()),
            (1.0, 0.0, net.get(2, 0).unwrap()),
            (0.0, 1.0, net.get(0, 2).unwrap()),
            (1.0, 1.0, net.get(2, 2).unwrap()),
        ] {
            assert_relative_eq!(
                patch.eval(u, v).unwrap(),
                Point3::new(corner.x, corner.y, corner.z),
                epsilon = 1e-6
            );
        }
    }

    /// Acceptance (T8.6): patch normals match finite-difference normals.
    #[test]
    fn bezier_patch_normals_match_finite_differences() {
        let patch = BezierPatch::new(bumped());
        let h = 1e-3;
        for i in 1..8 {
            for j in 1..8 {
                let (u, v) = (i as Real / 8.0, j as Real / 8.0);
                let analytic = patch.normal(u, v).unwrap();
                let du = (patch.eval(u + h, v).unwrap() - patch.eval(u - h, v).unwrap()) / (2.0 * h);
                let dv = (patch.eval(u, v + h).unwrap() - patch.eval(u, v - h).unwrap()) / (2.0 * h);
                let numeric = du.cross(&dv).normalize();
                assert!(
                    (analytic - numeric).norm() < 1e-4,
                    "normal at ({u}, {v}): analytic {analytic:?} numeric {numeric:?}"
                );
            }
        }
    }

    #[test]
    fn nurbs_patch_normals_match_finite_differences() {
        let patch = NurbsPatch::new(bumped(), 2, 2);
        let h = 1e-3;
        for i in 1..8 {
            for j in 1..8 {
                let (u, v) = (i as Real / 8.0, j as Real / 8.0);
                let analytic = patch.normal(u, v).unwrap();
                let du = (patch.eval(u + h, v).unwrap() - patch.eval(u - h, v).unwrap()) / (2.0 * h);
                let dv = (patch.eval(u, v + h).unwrap() - patch.eval(u, v - h).unwrap()) / (2.0 * h);
                let numeric = du.cross(&dv).normalize();
                assert!(
                    (analytic - numeric).norm() < 1e-4,
                    "normal at ({u}, {v}): analytic {analytic:?} numeric {numeric:?}"
                );
            }
        }
    }

    /// A NURBS patch whose degrees saturate its control net is the Bézier patch
    /// over the same net — the standard equivalence, and a cross-check between
    /// two independent evaluators.
    #[test]
    fn a_saturated_nurbs_patch_is_the_bezier_patch() {
        let bezier = BezierPatch::new(bumped());
        let nurbs = NurbsPatch::new(bumped(), 2, 2);
        for i in 0..=8 {
            for j in 0..=8 {
                let (u, v) = (i as Real / 8.0, j as Real / 8.0);
                assert_relative_eq!(
                    bezier.eval(u, v).unwrap(),
                    nurbs.eval(u, v).unwrap(),
                    epsilon = 1e-5
                );
            }
        }
    }

    #[test]
    fn a_flat_patch_has_a_constant_normal() {
        let flat = CtrlPointMatrix::from_point_rows(vec![
            vec![Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 2.0, 0.0)],
            vec![Point3::new(3.0, 0.0, 0.0), Point3::new(3.0, 2.0, 0.0)],
        ])
        .unwrap();
        let patch = BezierPatch::new(flat);
        for (u, v) in [(0.0, 0.0), (0.5, 0.5), (1.0, 0.25)] {
            assert_relative_eq!(patch.normal(u, v).unwrap(), Vec3::z(), epsilon = 1e-6);
        }
    }

    #[test]
    fn a_degenerate_control_net_is_rejected() {
        let single = CtrlPointMatrix::from_point_rows(vec![vec![Point3::origin()]]).unwrap();
        assert!(matches!(
            BezierPatch::new(single).eval(0.0, 0.0),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn a_bad_knot_vector_is_rejected() {
        let error = NurbsPatch::with_knots(
            bumped(),
            2,
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0],
        )
        .unwrap_err();
        assert!(matches!(error, Error::BadKnotVector(_)));
    }
}
