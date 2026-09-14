//! The drawer that measures instead of building — surface area, volume,
//! bounding box and segment count, without allocating a mesh.
//!
//! Original to this port; upstream ships only the scene-graph drawer. It is
//! nonetheless part of the CeCILL-C crate and built on translated code; see
//! crates/plantgl/LICENSE.
//!
//! # Why it exists
//!
//! Surface area is a *gameplay* input: leaf area is a principled harvest-yield
//! driver, tying the reward to the visible phenotype rather than to a genotype
//! scalar. Yield therefore has to be computable for every plant in a garden,
//! on load and on harvest — and building, tessellating and throwing away a
//! mesh to read one number off it is the wrong shape of work.
//!
//! This drawer keeps five running sums and a box. It allocates nothing.
//!
//! # What the numbers mean
//!
//! Every tube, sweep, quad and polygon here is measured *as the polygon it is
//! drawn as*, at the section resolution it was drawn with — a cylinder of
//! eight slices contributes the lateral area of a regular octagonal prism, not
//! `2πrl`. That is deliberate: it makes this drawer agree with
//! [`MeshDrawer`](crate::modelling::MeshDrawer) and
//! [`SceneDrawer`](crate::modelling::SceneDrawer) to floating-point precision
//! on the same command sequence, which is what makes the three
//! interchangeable.
//!
//! The one exception is [`TurtleDrawer::sphere`], whose UV tessellation has no
//! tidy closed form; it contributes `4πr²` and `4πr³/3`, which its mesh
//! converges to from below. Upstream's `SurfComputer` reports the closed form
//! for a sphere too.
//!
//! Volume counts what each shape *encloses*. A turtle's tubes are drawn open
//! — upstream never caps them — so their meshes bound no volume and
//! [`crate::algo::measure::volume`] rightly refuses them; the volume of the
//! stem is still the thing a yield model wants, so it is summed here as though
//! the ends were closed. Flat shapes (quad, polygon, circle) enclose nothing
//! and add nothing.

use crate::algo::bbox::BoundingBox;
use crate::error::Result;
use crate::math::{Point3, Real, Vec3, EPSILON};
use crate::modelling::drawer::{DrawCtx, TurtleDrawer};
use crate::modelling::geometry::slices;
use crate::scenegraph::curve::{Curve2D, Curve2DRef, ParametricCurve};
use crate::scenegraph::geometry::Geometry;

/// What a turtle program drew, in numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measures {
    /// Total surface area of everything drawn.
    pub surface_area: Real,
    /// Total enclosed volume; see the module docs for what "enclosed" means
    /// for an open tube.
    pub volume: Real,
    /// The box enclosing everything drawn, or `None` if nothing was.
    pub bbox: Option<BoundingBox>,
    /// How many shapes were drawn.
    pub shape_count: usize,
    /// How many axis segments were drawn — `F`s and generalized-cylinder
    /// rings, the measure of how much *stem* there is.
    pub segment_count: usize,
}

impl Default for Measures {
    fn default() -> Self {
        Self {
            surface_area: 0.0,
            volume: 0.0,
            bbox: None,
            shape_count: 0,
            segment_count: 0,
        }
    }
}

/// A [`TurtleDrawer`] that accumulates [`Measures`].
#[derive(Debug, Clone, Default)]
pub struct MeasureDrawer {
    measures: Measures,
    samples: u32,
}

impl MeasureDrawer {
    pub fn new() -> Self {
        Self {
            measures: Measures::default(),
            samples: crate::algo::discretize::DiscretizeCtx::DEFAULT_CURVE_SAMPLES,
        }
    }

    /// What has been drawn so far.
    pub fn measures(&self) -> Measures {
        self.measures
    }

    pub fn surface_area(&self) -> Real {
        self.measures.surface_area
    }

    pub fn volume(&self) -> Real {
        self.measures.volume
    }

    pub fn bbox(&self) -> Option<BoundingBox> {
        self.measures.bbox
    }

    fn add_point(&mut self, point: Point3) {
        match &mut self.measures.bbox {
            Some(bbox) => {
                bbox.extend_point(point);
            }
            none => *none = Some(BoundingBox::from_point(point)),
        }
    }

    /// The box of a ring: a circle of radius `r` about `centre` in the plane
    /// normal to `heading` reaches `r·√(1 − hᵢ²)` along each axis `i`.
    fn add_ring(&mut self, centre: Point3, heading: &Vec3, radius: Real) {
        let extent = Vec3::new(
            radius * (1.0 - heading.x * heading.x).max(0.0).sqrt(),
            radius * (1.0 - heading.y * heading.y).max(0.0).sqrt(),
            radius * (1.0 - heading.z * heading.z).max(0.0).sqrt(),
        );
        self.add_point(centre - extent);
        self.add_point(centre + extent);
    }

    fn shape_drawn(&mut self) {
        self.measures.shape_count += 1;
    }
}

/// The perimeter and enclosed area of a regular `n`-gon inscribed in the unit
/// circle — what a turtle's tubes are actually drawn as.
fn ngon(n: u32) -> (Real, Real) {
    let n = n.max(3) as Real;
    let perimeter = 2.0 * n * (std::f32::consts::PI / n).sin();
    let area = 0.5 * n * (std::f32::consts::TAU / n).sin();
    (perimeter, area)
}

/// The same two numbers for an arbitrary cross-section, sampled at the density
/// it would be swept at. Allocation-free: the points are consumed as they are
/// evaluated.
fn section_metrics(section: &Curve2D, samples: u32) -> Result<(Real, Real)> {
    let stride = section.resolved_stride(samples);
    let first = section.first_knot();
    let last = section.last_knot();
    let step = (last - first) / stride as Real;

    let start = section.eval(first)?;
    let mut previous = start;
    let mut perimeter = 0.0;
    let mut twice_area = 0.0;
    for i in 1..=stride {
        let u = if i == stride {
            last
        } else {
            first + step * i as Real
        };
        let point = section.eval(u)?;
        perimeter += (point - previous).norm();
        twice_area += previous.x * point.y - point.x * previous.y;
        previous = point;
    }
    // Close the ring. For a section that already returns to its start these
    // two terms are zero, so an open and a closed profile are both handled.
    perimeter += (start - previous).norm();
    twice_area += previous.x * start.y - start.x * previous.y;
    Ok((perimeter, twice_area.abs() / 2.0))
}

/// The lateral area and volume of one swept segment between two rings of
/// radius `r0` and `r1`, `length` apart along the axis.
///
/// `perimeter` and `area` are the section's, at unit radius; `apothem` is how
/// far a face's mid-line sits from the axis, which decides the slant height of
/// the trapezoid that face is.
fn segment_measures(
    perimeter: Real,
    area: Real,
    apothem: Real,
    r0: Real,
    r1: Real,
    length: Real,
) -> (Real, Real) {
    let slant = (length * length + ((r0 - r1) * apothem).powi(2)).sqrt();
    let lateral = 0.5 * perimeter * (r0 + r1) * slant;
    let volume = area * length.abs() * (r0 * r0 + r0 * r1 + r1 * r1) / 3.0;
    (lateral, volume)
}

impl TurtleDrawer for MeasureDrawer {
    fn reset(&mut self) {
        self.measures = Measures::default();
    }

    fn cylinder(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        self.frustum(ctx, length, radius, radius, section_resolution)
    }

    fn frustum(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        base_radius: Real,
        top_radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        let length = length * ctx.scaling.z;
        if length.abs() <= EPSILON {
            return Ok(());
        }
        let n = slices(section_resolution) as u32;
        let (perimeter, area) = ngon(n);
        let apothem = (std::f32::consts::PI / n as Real).cos();
        let (lateral, volume) =
            segment_measures(perimeter, area, apothem, base_radius, top_radius, length);
        self.measures.surface_area += lateral;
        self.measures.volume += volume;
        self.measures.segment_count += 1;
        self.shape_drawn();

        let frame = ctx.frame;
        let top = frame.position + frame.heading * length;
        self.add_ring(frame.position, &frame.heading, base_radius);
        self.add_ring(top, &frame.heading, top_radius);
        Ok(())
    }

    fn generalized_cylinder(
        &mut self,
        ctx: &DrawCtx,
        points: &[Point3],
        _lefts: &[Vec3],
        radii: &[Real],
        cross_section: Curve2DRef,
        _ccw: bool,
        section_resolution: u32,
    ) -> Result<()> {
        let _ = (ctx, section_resolution);
        if points.len() < 2 {
            return Ok(());
        }
        let (perimeter, area) = section_metrics(cross_section.as_ref(), self.samples)?;
        // A section's mean apothem: the radius of the circle of the same
        // perimeter is `perimeter / 2π`, and the slant correction is second
        // order in the radius difference, so this is exact for the circular
        // default and a good approximation otherwise.
        let apothem = perimeter / std::f32::consts::TAU;

        for i in 0..points.len() - 1 {
            let length = (points[i + 1] - points[i]).norm();
            if length <= EPSILON {
                continue;
            }
            let r0 = radii.get(i).copied().unwrap_or(1.0);
            let r1 = radii.get(i + 1).copied().unwrap_or(r0);
            let (lateral, volume) = segment_measures(perimeter, area, apothem, r0, r1, length);
            self.measures.surface_area += lateral;
            self.measures.volume += volume;
            self.measures.segment_count += 1;
        }

        for (i, point) in points.iter().enumerate() {
            let heading = if i + 1 < points.len() {
                points[i + 1] - *point
            } else {
                *point - points[i - 1]
            };
            let heading = heading.try_normalize(EPSILON).unwrap_or_else(Vec3::z);
            self.add_ring(*point, &heading, radii.get(i).copied().unwrap_or(1.0));
        }
        self.shape_drawn();
        Ok(())
    }

    fn sphere(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let _ = section_resolution;
        if radius <= EPSILON {
            return Ok(());
        }
        let radius = radius * ctx.scaling.x;
        self.measures.surface_area += 2.0 * std::f32::consts::TAU * radius * radius;
        self.measures.volume += 2.0 * std::f32::consts::TAU * radius * radius * radius / 3.0;
        self.shape_drawn();
        let extent = Vec3::new(radius, radius, radius);
        self.add_point(ctx.frame.position - extent);
        self.add_point(ctx.frame.position + extent);
        Ok(())
    }

    fn circle(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        if radius < EPSILON {
            return Ok(());
        }
        let radius = radius * ctx.scaling.x;
        let (_, area) = ngon(slices(section_resolution) as u32);
        self.measures.surface_area += area * radius * radius;
        self.shape_drawn();
        self.add_ring(ctx.frame.position, &ctx.frame.heading, radius);
        Ok(())
    }

    fn box3(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let length = length * ctx.scaling.z;
        if length.abs() <= EPSILON {
            return Ok(());
        }
        let (r0, r1) = (bottom_radius, top_radius);
        let slant = (length * length + (r0 - r1) * (r0 - r1)).sqrt();
        // Four trapezoidal sides and two square caps.
        self.measures.surface_area += 4.0 * (r0 + r1) * slant + 4.0 * (r0 * r0 + r1 * r1);
        self.measures.volume += 4.0 * length.abs() * (r0 * r0 + r0 * r1 + r1 * r1) / 3.0;
        self.measures.segment_count += 1;
        self.shape_drawn();

        let frame = ctx.frame;
        let top = frame.position + frame.heading * length;
        for (centre, radius) in [(frame.position, r0), (top, r1)] {
            for sign_left in [-1.0, 1.0] {
                for sign_up in [-1.0, 1.0] {
                    self.add_point(
                        centre + frame.left * (radius * sign_left) + frame.up * (radius * sign_up),
                    );
                }
            }
        }
        Ok(())
    }

    fn quad(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let length = length * ctx.scaling.z;
        self.measures.surface_area += (bottom_radius + top_radius) * length.abs();
        self.shape_drawn();

        let frame = ctx.frame;
        let top = frame.position + frame.heading * length;
        for (centre, radius) in [(frame.position, bottom_radius), (top, top_radius)] {
            self.add_point(centre + frame.left * radius);
            self.add_point(centre - frame.left * radius);
        }
        Ok(())
    }

    fn polygon(&mut self, ctx: &DrawCtx, points: &[Point3], concave_test: bool) -> Result<()> {
        let _ = (ctx, concave_test);
        if points.len() < 3 {
            return Ok(());
        }
        // The fan's area. For a convex outline this is the mesh's area
        // exactly; for a concave one an ear clip triangulates the same region,
        // so the sum is the same up to the sign of the reflex ears, which the
        // absolute value below takes care of.
        let mut twice_area = Vec3::zeros();
        for i in 1..points.len() - 1 {
            twice_area += (points[i] - points[0]).cross(&(points[i + 1] - points[0]));
        }
        self.measures.surface_area += twice_area.norm() / 2.0;
        self.shape_drawn();
        for point in points {
            self.add_point(*point);
        }
        Ok(())
    }

    fn custom_geometry(&mut self, ctx: &DrawCtx, geometry: &Geometry, scale: Real) -> Result<()> {
        // A surface template is arbitrary geometry, so there is no closed form
        // to add: it is discretised once, measured, and dropped. This is the
        // one command that allocates, and it is the one that has to.
        let placed = crate::modelling::geometry::custom_geometry(ctx, geometry, scale);
        let Some(placed) = placed else {
            return Ok(());
        };
        let model = crate::algo::discretize::discretize(&placed)?;
        self.measures.surface_area += crate::algo::measure::surface_area(&model)?;
        if model.is_solid() {
            self.measures.volume += crate::algo::measure::volume(&model)?;
        }
        self.shape_drawn();
        for point in model.points() {
            self.add_point(*point);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Frame;
    use crate::modelling::drawer::IdPair;
    use approx::assert_relative_eq;

    fn ctx(frame: &Frame) -> DrawCtx<'_> {
        DrawCtx {
            ids: IdPair::none(),
            appearance: None,
            frame,
            scaling: Vec3::new(1.0, 1.0, 1.0),
        }
    }

    #[test]
    fn a_cylinder_measures_as_the_prism_it_is_drawn_as() {
        let frame = Frame::default();
        let mut drawer = MeasureDrawer::new();
        drawer.cylinder(&ctx(&frame), 2.0, 0.5, 8).unwrap();

        let (perimeter, area) = ngon(8);
        assert_relative_eq!(
            drawer.surface_area(),
            perimeter * 0.5 * 2.0,
            epsilon = 1e-5
        );
        assert_relative_eq!(drawer.volume(), area * 0.25 * 2.0, epsilon = 1e-5);
        assert_eq!(drawer.measures().segment_count, 1);

        let bbox = drawer.bbox().unwrap();
        assert_relative_eq!(bbox.lower_left, Point3::new(-0.5, -0.5, 0.0), epsilon = 1e-6);
        assert_relative_eq!(bbox.upper_right, Point3::new(0.5, 0.5, 2.0), epsilon = 1e-6);
    }

    #[test]
    fn a_fine_cylinder_converges_on_the_closed_form() {
        let frame = Frame::default();
        let mut drawer = MeasureDrawer::new();
        drawer.cylinder(&ctx(&frame), 1.0, 1.0, 256).unwrap();
        assert_relative_eq!(
            drawer.surface_area(),
            std::f32::consts::TAU,
            max_relative = 1e-4
        );
        // The inscribed 256-gon is short of the circle by ~(2π/n)²/6.
        assert_relative_eq!(drawer.volume(), std::f32::consts::PI, max_relative = 2e-4);
    }

    #[test]
    fn a_cone_has_a_third_of_the_cylinders_volume() {
        let frame = Frame::default();
        let mut drawer = MeasureDrawer::new();
        drawer.frustum(&ctx(&frame), 1.0, 1.0, 0.0, 256).unwrap();
        assert_relative_eq!(
            drawer.volume(),
            std::f32::consts::PI / 3.0,
            max_relative = 1e-3
        );
    }

    #[test]
    fn a_sphere_uses_the_closed_form() {
        let frame = Frame::default();
        let mut drawer = MeasureDrawer::new();
        drawer.sphere(&ctx(&frame), 2.0, 8).unwrap();
        assert_relative_eq!(
            drawer.surface_area(),
            4.0 * std::f32::consts::PI * 4.0,
            epsilon = 1e-4
        );
        assert_relative_eq!(
            drawer.volume(),
            4.0 * std::f32::consts::PI * 8.0 / 3.0,
            epsilon = 1e-4
        );
    }

    #[test]
    fn a_flat_shape_adds_no_volume() {
        let frame = Frame::default();
        let mut drawer = MeasureDrawer::new();
        drawer.quad(&ctx(&frame), 2.0, 0.5, 0.5).unwrap();
        assert_relative_eq!(drawer.surface_area(), 2.0, epsilon = 1e-6);
        assert_eq!(drawer.volume(), 0.0);
    }

    #[test]
    fn a_polygon_measures_its_own_area() {
        let frame = Frame::default();
        let mut drawer = MeasureDrawer::new();
        let square = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 2.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
        ];
        drawer.polygon(&ctx(&frame), &square, false).unwrap();
        assert_relative_eq!(drawer.surface_area(), 4.0, epsilon = 1e-6);
    }

    #[test]
    fn an_arbitrary_section_is_measured_from_the_curve() {
        use crate::scenegraph::curve::Polyline2D;
        let section = Curve2D::from(Polyline2D::circle(1.0, 16));
        let (perimeter, area) = section_metrics(&section, 30).unwrap();
        let (expected_perimeter, expected_area) = ngon(16);
        assert_relative_eq!(perimeter, expected_perimeter, epsilon = 1e-5);
        assert_relative_eq!(area, expected_area, epsilon = 1e-5);
    }

    #[test]
    fn nothing_drawn_leaves_no_box() {
        assert!(MeasureDrawer::new().bbox().is_none());
        assert_eq!(MeasureDrawer::new().measures().shape_count, 0);
    }
}
