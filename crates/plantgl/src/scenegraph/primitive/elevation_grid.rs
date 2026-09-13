//! The elevation grid.
//!
//! Ported from PlantGL
//! `src/cpp/plantgl/scenegraph/geometry/elevationgrid.{h,cpp}` and
//! `src/cpp/plantgl/tool/util_array2.h`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point3, Real};

use super::is_positive;

/// A rectangular array of heights — upstream's `RealArray2` in the one role
/// this crate uses it for.
///
/// **Upstream indexes it `(row, column)` but names the dimensions the other way
/// round**: `ElevationGrid::getXDim()` returns the *column* count and
/// `getYDim()` the *row* count, and `getPointAt(i, j)` reads `heights(j, i)`.
/// The naming here follows the grid's axes — [`HeightField::x_dim`] is the
/// number of samples along x — and [`HeightField::get`] takes them in that
/// order, so the transposition lives in one place instead of at every call.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeightField {
    x_dim: usize,
    y_dim: usize,
    /// Row-major, `y_dim` rows of `x_dim` values, matching upstream's storage.
    values: Vec<Real>,
}

impl HeightField {
    /// Builds from `y_dim` rows of `x_dim` heights each, given row-major.
    pub fn new(x_dim: usize, y_dim: usize, values: Vec<Real>) -> Result<Self> {
        if x_dim < 2 || y_dim < 2 {
            return Err(Error::degenerate(format!(
                "an elevation grid needs at least 2×2 samples, got {x_dim}×{y_dim}"
            )));
        }
        if values.len() != x_dim * y_dim {
            return Err(Error::invalid_index(format!(
                "{} heights for a {x_dim}×{y_dim} grid",
                values.len()
            )));
        }
        Ok(Self {
            x_dim,
            y_dim,
            values,
        })
    }

    /// Builds from rows given outermost-first, as they would be written down.
    pub fn from_rows(rows: Vec<Vec<Real>>) -> Result<Self> {
        let y_dim = rows.len();
        let x_dim = rows.first().map_or(0, Vec::len);
        if rows.iter().any(|row| row.len() != x_dim) {
            return Err(Error::invalid_index("elevation grid rows differ in length"));
        }
        Self::new(x_dim, y_dim, rows.into_iter().flatten().collect())
    }

    /// A flat grid of the given dimensions.
    pub fn flat(x_dim: usize, y_dim: usize) -> Result<Self> {
        Self::new(x_dim, y_dim, vec![0.0; x_dim * y_dim])
    }

    /// `getXDim()` — samples along x, upstream's column count.
    pub fn x_dim(&self) -> usize {
        self.x_dim
    }

    /// `getYDim()` — samples along y, upstream's row count.
    pub fn y_dim(&self) -> usize {
        self.y_dim
    }

    /// `getHeightAt(i, j)`.
    pub fn get(&self, i: usize, j: usize) -> Option<Real> {
        if i >= self.x_dim || j >= self.y_dim {
            return None;
        }
        self.values.get(j * self.x_dim + i).copied()
    }

    /// The heights in upstream's row-major order.
    pub fn values(&self) -> &[Real] {
        &self.values
    }
}

/// Upstream's `ElevationGrid` — a height field sampled on a regular xy lattice.
///
/// Sample `(i, j)` sits at `(i * x_spacing, j * y_spacing, height(i, j))`, so
/// the grid's lower corner is at the origin and it grows into +x and +y.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ElevationGrid {
    /// The `HeightList` field.
    pub heights: Arc<HeightField>,
    /// The `XSpacing` field.
    pub x_spacing: Real,
    /// The `YSpacing` field.
    pub y_spacing: Real,
    /// The `CCW` field — the winding the emitted triangles carry.
    pub ccw: bool,
}

impl ElevationGrid {
    /// `ElevationGrid::DEFAULT_X_SPACING`.
    pub const DEFAULT_X_SPACING: Real = 1.0;
    /// `ElevationGrid::DEFAULT_Y_SPACING`.
    pub const DEFAULT_Y_SPACING: Real = 1.0;
    /// `Patch::DEFAULT_CCW`.
    pub const DEFAULT_CCW: bool = true;

    /// `ElevationGrid(heights, xSpacing, ySpacing, ccw)`.
    pub fn new(heights: HeightField, x_spacing: Real, y_spacing: Real, ccw: bool) -> Self {
        Self {
            heights: Arc::new(heights),
            x_spacing,
            y_spacing,
            ccw,
        }
    }

    /// A grid at upstream's default unit spacing.
    pub fn from_heights(heights: HeightField) -> Self {
        Self::new(
            heights,
            Self::DEFAULT_X_SPACING,
            Self::DEFAULT_Y_SPACING,
            Self::DEFAULT_CCW,
        )
    }

    /// `getPointAt(i, j)`.
    pub fn point_at(&self, i: usize, j: usize) -> Option<Point3> {
        let height = self.heights.get(i, j)?;
        Some(Point3::new(
            i as Real * self.x_spacing,
            j as Real * self.y_spacing,
            height,
        ))
    }

    /// `getXSize()` — the grid's full extent along x.
    pub fn x_size(&self) -> Real {
        (self.heights.x_dim() - 1) as Real * self.x_spacing
    }

    /// `getYSize()`.
    pub fn y_size(&self) -> Real {
        (self.heights.y_dim() - 1) as Real * self.y_spacing
    }

    /// `isValid()`.
    pub fn is_valid(&self) -> Result<()> {
        if !is_positive(self.x_spacing) || !is_positive(self.y_spacing) {
            return Err(Error::degenerate(format!(
                "elevation grid spacing must be positive, got {}×{}",
                self.x_spacing, self.y_spacing
            )));
        }
        if self.heights.values().iter().any(|h| !h.is_finite()) {
            return Err(Error::degenerate("elevation grid has a non-finite height"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn ramp() -> HeightField {
        // Three samples along x, two along y.
        HeightField::from_rows(vec![vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 2.0]]).unwrap()
    }

    #[test]
    fn dimensions_follow_the_grids_own_axes() {
        let heights = ramp();
        assert_eq!(heights.x_dim(), 3);
        assert_eq!(heights.y_dim(), 2);
        assert_eq!(heights.get(2, 1), Some(2.0));
        assert_eq!(heights.get(3, 0), None);
        assert_eq!(heights.get(0, 2), None);
    }

    #[test]
    fn points_are_laid_out_on_the_spacing_lattice() {
        let grid = ElevationGrid::new(ramp(), 2.0, 5.0, true);
        assert_relative_eq!(
            grid.point_at(2, 1).unwrap(),
            Point3::new(4.0, 5.0, 2.0),
            epsilon = 1e-6
        );
        assert_relative_eq!(grid.x_size(), 4.0);
        assert_relative_eq!(grid.y_size(), 5.0);
        assert_eq!(grid.point_at(9, 0), None);
    }

    #[test]
    fn a_grid_needs_at_least_two_samples_per_axis() {
        assert!(HeightField::new(1, 4, vec![0.0; 4]).is_err());
        assert!(HeightField::flat(2, 2).is_ok());
    }

    #[test]
    fn mismatched_lengths_are_rejected() {
        assert!(HeightField::new(2, 2, vec![0.0; 3]).is_err());
        assert!(HeightField::from_rows(vec![vec![0.0, 1.0], vec![0.0]]).is_err());
    }

    #[test]
    fn validity_rejects_bad_spacing_and_heights() {
        assert!(ElevationGrid::from_heights(ramp()).is_valid().is_ok());
        assert!(ElevationGrid::new(ramp(), 0.0, 1.0, true).is_valid().is_err());

        let nan = HeightField::new(2, 2, vec![0.0, 0.0, 0.0, Real::NAN]).unwrap();
        assert!(ElevationGrid::from_heights(nan).is_valid().is_err());
    }
}
