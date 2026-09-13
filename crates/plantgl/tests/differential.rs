//! Differential testing against upstream PlantGL.
//!
//! Part of the `plantgl` crate and therefore licensed CeCILL-C; see
//! crates/plantgl/LICENSE. Nothing here is translated from PlantGL — this
//! checks the translation against the original.
//!
//! Because the CeCILL-C decision lets us read *and run* upstream, the strongest
//! available check on a translation is to drive the same scene through both and
//! compare. That is what this does, and it is worth far more than analytic
//! spot-checks on the awkward primitives — `Revolution`, `Swung`,
//! `ElevationGrid`, `Paraboloid` — where there is no closed form to check
//! against at all.
//!
//! # How it runs
//!
//! `tools/differential/upstream_measure.py` drives a conda-installed PlantGL
//! and writes `reference.json`, which is committed. This test reads that file
//! and needs **no Python, no conda and no network** — so the differential gate
//! runs on every `cargo test` rather than being an optional job somebody
//! remembers to trigger. Regenerating the reference is the manual step, and
//! `tools/differential/README.md` documents it.
//!
//! # What is compared, and how tightly
//!
//! Not everything upstream reports is comparable to what we report, and
//! pretending otherwise would turn this into a test that passes by being
//! loose. Three kinds of claim, with three tolerances:
//!
//! 1. **Mesh topology** — point, face and triangle counts, the `solid` and
//!    `ccw` flags, the bounding box. Both sides run their discretizer, so these
//!    must match *exactly* (the bbox to float tolerance). This is the check
//!    that catches a mis-translated index expression.
//! 2. **Mesh-measured area and volume** — where upstream's computer
//!    discretises too, the numbers describe the same mesh and must agree to
//!    1e-4 relative.
//! 3. **Analytic area and volume** — where upstream's computer returns the
//!    closed form of the ideal surface instead (see
//!    `tools/differential/upstream_measure.py`), our discretised measure can
//!    only *converge* to it, so the check is the issue's 1% at the fine slice
//!    count and a looser bound at the coarse one.
//!
//! The two documented divergences are asserted as divergences, with their
//! predicted magnitude, rather than being excluded — see [`POLE_COLLAPSE`].

use std::collections::BTreeMap;

use plantgl::algo::bbox::bounding_box;
use plantgl::algo::discretize::{discretize_with, DiscretizeCtx, Explicit};
use plantgl::algo::measure::{surface_area, volume};
use plantgl::algo::tessellate::tessellate;
use plantgl::math::{Point2, Point3, Real, Vec2, Vec3, Vec4};
use plantgl::scenegraph::curve::{
    BezierCurve, BezierCurve2D, BezierPatch, CtrlPointMatrix, Curve2D, Curve3D, NurbsCurve,
    NurbsCurve2D, NurbsPatch, ParametricCurve, Polyline2D,
};
use plantgl::scenegraph::mesh::Polyline;
use plantgl::scenegraph::primitive::{
    Box3, Cone, Cylinder, Disc, ElevationGrid, Extrusion, Frustum, HeightField, Paraboloid,
    Revolution, Sphere,
};
use plantgl::Geometry;

/// The reference upstream produced, as committed.
const REFERENCE: &str = include_str!("reference.json");

/// Topology must match exactly; the bounding box only to float tolerance.
const BBOX_TOLERANCE: Real = 1e-4;

/// Where both sides measure the same mesh, they must agree to this.
const MESH_TOLERANCE: Real = 1e-4;

/// Where upstream reports the ideal surface, the issue's "within 1%" applies
/// at the fine slice count.
const CONVERGENCE_TOLERANCE: Real = 0.01;

/// At the coarse slice count the polygonal shortfall is real and large, so
/// convergence is checked only loosely there; the fine count carries the
/// issue's 1% claim. Volume gets the looser bound of the two because its error
/// goes roughly as the cube of the linear one — an 8x8 sphere is ~6% short in
/// area and ~13% short in volume.
const COARSE_AREA_TOLERANCE: Real = 0.12;
const COARSE_VOLUME_TOLERANCE: Real = 0.20;

/// The port collapses a swept pole to one shared apex where upstream sweeps
/// the strip through the axis; see `Discretizer::sweep_profiles`.
///
/// The difference is exactly predictable, which is what makes it safe to
/// accept: upstream emits one zero-area triangle per slice per pole, and the
/// reference file records how many it emitted, so the expected counts are
/// derived from upstream's own numbers rather than hardcoded.
struct PoleCollapse {
    /// Poles collapsed for this case: 0, 1 or 2.
    poles: usize,
}

/// Cases where the port collapses swept poles. Everything else must match
/// upstream's counts exactly.
///
/// `frustum_cone` is deliberately *not* here: a frustum of taper 0 collapses
/// its top ring to a point too, but it goes through upstream's own
/// `process(Frustum*)` code path, which the port translates verbatim — so the
/// port reproduces upstream's degenerate cap triangles exactly, and the
/// reference file confirms it does.
const POLE_COLLAPSE: &[(&str, PoleCollapse)] =
    &[("revolution_closed", PoleCollapse { poles: 2 })];

fn pole_collapse(case: &str) -> usize {
    POLE_COLLAPSE
        .iter()
        .find(|(name, _)| *name == case)
        .map_or(0, |(_, c)| c.poles)
}

/// The stride `upstream_measure.py` pins every curve case at.
const CURVE_STRIDE: u32 = 24;

/// How closely a sampled curve must agree with upstream's.
///
/// Upstream evaluates in `f32` throughout; the port widens knots and basis
/// functions to `f64` and narrows only the finished point, so on these curves
/// upstream is the less accurate of the two by around `1e-6` on coordinates of
/// order 5. `1e-4` is four orders looser than that and four orders tighter than
/// any mistranslation — a wrong span, a transposed control point or a dropped
/// weight moves a point by a tenth of the curve, not a ten-thousandth.
const CURVE_TOLERANCE: Real = 1e-4;

/// The third documented divergence, after the collapsed poles and the errored
/// volume: **the port's swept frames are rotation-minimising and upstream's are
/// not.**
///
/// Upstream's `Extrusion::getNextFrameAt` crosses the previous binormal with
/// the new tangent — the projection method, second-order accurate in the twist.
/// The port carries frames by double reflection (Wang et al. 2008), which is
/// fourth-order. Both are rotation-minimising in the limit and both produce the
/// *same tangent* at every ring, so the two meshes differ only by a rotation of
/// each ring about its own axis.
///
/// That makes the difference bounded rather than open-ended, and bounded by
/// something derivable rather than a fudge factor. The cross-section here is a
/// circle sampled as an *n*-gon: rotating an *n*-gon inscribed in a circle of
/// radius `r` moves its outline by at most the sagitta `r(1 - cos(π/n))`, so no
/// vertex can leave that shell and the bounding box cannot move further than
/// it. Returns that bound, or `None` for the cases where the two frame chains
/// provably coincide — every straight axis, where there is no rotation to
/// minimise and the meshes must match exactly.
fn frame_divergence_bound(case: &str, slices: u8) -> Option<Real> {
    (case == "extrusion_helix").then(|| {
        const CROSS_SECTION_RADIUS: Real = 0.15;
        CROSS_SECTION_RADIUS * (1.0 - (std::f32::consts::PI / slices as Real).cos())
    })
}

/// The same divergence, as a relative area tolerance.
///
/// Both meshes inscribe the same tube in the same rings, so the areas differ
/// only through how each ring's *n*-gon lines up with its neighbour's. That is
/// a second-order effect in the per-step twist, and 1% is well inside it while
/// still catching a mesh that is wrong rather than merely rotated.
const FRAME_DIVERGENCE_AREA_TOLERANCE: Real = 0.01;

/// Cases where **upstream's own analytic formula is wrong**, with the correct
/// closed form to compare against instead.
///
/// The differential harness found both of these, which is the point of having
/// it. Each is a dimensional error — a length returned where an area belongs —
/// so neither is a matter of convention:
///
/// - `SurfComputer::process(Disc*)` is `__result = GEOM_TWO_PI * radius`, with
///   the comment `// 2 PI r`. That is the disc's **circumference**. The area is
///   `π r²`. A unit disc hides it (2π vs π is just a factor of two), which is
///   why `disc_r3` is in the case list: upstream reports 18.85 = 2π·3 for a
///   radius-3 disc whose area is 28.27.
/// - `SurfComputer::process(Frustum*)`'s solid branch adds its end caps as
///   `π(r + q)` where the areas are `π(r² + q²)`. Its non-solid branch is
///   correct, which is why `frustum_open` matches us exactly and
///   `frustum_solid` does not. `frustum_cone` has `r = 1`, where `π r` and
///   `π r²` coincide, so the defect is invisible there and that case is
///   compared against upstream as normal.
///
/// The port computes area by measuring the mesh, so it converges to the
/// correct value and cannot reproduce either bug even if we wanted it to.
/// These entries are checked both ways: our number must approach the correct
/// closed form, and upstream's must still be wrong. If an upstream rebase
/// fixes one, [`known_upstream_area_defects_are_still_present`] fails and the
/// entry should be removed.
fn upstream_area_defect(case: &str) -> Option<Real> {
    let disc = |r: Real| std::f32::consts::PI * r * r;
    let frustum = |r: Real, h: Real, taper: Real| {
        let q = r * taper;
        let slant = ((r - q) * (r - q) + h * h).sqrt();
        std::f32::consts::PI * ((r + q) * slant + r * r + q * q)
    };
    Some(match case {
        "disc_unit" => disc(1.0),
        "disc_r3" => disc(3.0),
        "frustum_solid" => frustum(2.0, 3.0, 0.5),
        _ => return None,
    })
}

// --- The case list, mirroring tools/differential/upstream_measure.py --------

fn profile(points: &[(Real, Real)]) -> plantgl::scenegraph::curve::Curve2DRef {
    Curve2D::from(Polyline2D::new(
        points.iter().map(|(x, y)| Point2::new(*x, *y)).collect(),
    ))
    .into_ref()
}

fn straight_axis(height: Real, segments: usize) -> plantgl::scenegraph::curve::Curve3DRef {
    Curve3D::from(Polyline::new(
        (0..=segments)
            .map(|i| Point3::new(0.0, 0.0, height * i as Real / segments as Real))
            .collect(),
    ))
    .into_ref()
}

/// The helix `upstream_measure.py`'s `helix_axis` builds.
fn helix_axis(turns: Real, segments: usize) -> plantgl::scenegraph::curve::Curve3DRef {
    let (a, b) = (1.0, 0.35);
    Curve3D::from(Polyline::new(
        (0..=segments)
            .map(|i| {
                let t = turns * std::f32::consts::TAU * i as Real / segments as Real;
                Point3::new(a * t.cos(), a * t.sin(), b * t)
            })
            .collect(),
    ))
    .into_ref()
}

fn circle_section(radius: Real, slices: u8) -> plantgl::scenegraph::curve::Curve2DRef {
    Curve2D::from(Polyline2D::circle(radius, slices)).into_ref()
}

/// `PATCH_ROWS` in `upstream_measure.py`, as a `[u][v]` control net.
fn patch_net() -> CtrlPointMatrix {
    let rows = [
        [(0.0, 0.0, 0.0), (0.0, 1.0, 0.8), (0.0, 2.0, -0.2), (0.0, 3.0, 0.0)],
        [(1.0, 0.0, 0.5), (1.0, 1.0, 2.0), (1.0, 2.0, 1.0), (1.0, 3.0, -0.4)],
        [(2.0, 0.0, -0.3), (2.0, 1.0, 0.9), (2.0, 2.0, 1.4), (2.0, 3.0, 0.2)],
        [(3.0, 0.0, 0.0), (3.0, 1.0, -0.6), (3.0, 2.0, 0.3), (3.0, 3.0, 0.0)],
    ];
    CtrlPointMatrix::from_point_rows(
        rows.iter()
            .map(|row| row.iter().map(|(x, y, z)| Point3::new(*x, *y, *z)).collect())
            .collect(),
    )
    .unwrap()
}

/// Every case, built at the given slice count. The names and the parameters
/// must match `CASES` in the Python script exactly; a name here with no
/// counterpart there fails the coverage test below.
fn build(case: &str, s: u8) -> Option<Geometry> {
    Some(match case {
        "box_unit" => Geometry::from(Box3::new(Vec3::new(0.5, 0.5, 0.5))),
        "box_oblong" => Geometry::from(Box3::new(Vec3::new(1.0, 2.0, 3.0))),
        "sphere_unit" => Geometry::from(Sphere::new(1.0, s, s)),
        "sphere_small" => Geometry::from(Sphere::new(0.25, s, s)),
        "cone_solid" => Geometry::from(Cone::new(1.0, 2.0, true, s)),
        "cone_open" => Geometry::from(Cone::new(1.0, 2.0, false, s)),
        "cylinder_solid" => Geometry::from(Cylinder::new(1.0, 2.0, true, s)),
        "cylinder_open" => Geometry::from(Cylinder::new(1.0, 2.0, false, s)),
        "frustum_solid" => Geometry::from(Frustum::new(2.0, 3.0, 0.5, true, s)),
        "frustum_open" => Geometry::from(Frustum::new(2.0, 3.0, 0.5, false, s)),
        "frustum_cone" => Geometry::from(Frustum::new(1.0, 2.0, 0.0, true, s)),
        "disc_unit" => Geometry::from(Disc::new(1.0, s)),
        "disc_r3" => Geometry::from(Disc::new(3.0, s)),
        "paraboloid_solid" => Geometry::from(Paraboloid::new(1.0, 2.0, 2.0, true, s, s)),
        "paraboloid_sharp" => Geometry::from(Paraboloid::new(1.0, 2.0, 0.5, true, s, s)),
        "revolution_tube" => Geometry::from(Revolution::new(
            profile(&[(2.0, 0.0), (1.5, 1.0), (1.0, 3.0)]),
            s,
        )),
        "revolution_closed" => Geometry::from(Revolution::new(
            profile(&[(0.0, 0.0), (1.0, 1.0), (1.0, 2.0), (0.0, 3.0)]),
            s,
        )),
        "elevation_grid_flat" => {
            Geometry::from(ElevationGrid::from_heights(HeightField::flat(5, 5).unwrap()))
        }
        "elevation_grid_bump" => {
            // Six samples along x, four along y — upstream's RealArray2 rows
            // are y and its columns x, which `HeightField::from_rows` matches.
            let rows: Vec<Vec<Real>> = (0..4)
                .map(|j| {
                    (0..6)
                        .map(|i| (i as Real * 0.7).sin() * (j as Real * 0.5).cos())
                        .collect()
                })
                .collect();
            Geometry::from(ElevationGrid::new(
                HeightField::from_rows(rows).unwrap(),
                0.5,
                2.0,
                true,
            ))
        }

        // --- Phase C (#19) --------------------------------------------------
        "bezier_patch_bump" => {
            Geometry::from(BezierPatch::new(patch_net()).with_strides(s as u32, s as u32))
        }
        "nurbs_patch_bump" => {
            Geometry::from(NurbsPatch::new(patch_net(), 3, 3).with_strides(s as u32, s as u32))
        }
        "extrusion_straight" => Geometry::from(Extrusion::new(
            straight_axis(2.0, 8),
            circle_section(0.5, s),
        )),
        "extrusion_straight_solid" => Geometry::from(
            Extrusion::new(straight_axis(2.0, 8), circle_section(0.5, s)).with_solid(true),
        ),
        "extrusion_open_section" => Geometry::from(Extrusion::new(
            straight_axis(2.0, 6),
            profile(&[(1.0, 0.0), (0.5, 0.8), (-0.5, 0.8), (-1.0, 0.0)]),
        )),
        "extrusion_tapered" => Geometry::from(
            Extrusion::new(straight_axis(3.0, 8), circle_section(1.0, s))
                .with_scale(vec![Vec2::new(1.0, 1.0), Vec2::new(0.25, 0.25)]),
        ),
        "extrusion_twisted" => Geometry::from(
            Extrusion::new(
                straight_axis(2.0, 8),
                profile(&[
                    (1.0, 0.0),
                    (0.3, 0.6),
                    (-1.0, 0.0),
                    (0.3, -0.6),
                    (1.0, 0.0),
                ]),
            )
            .with_orientation(vec![0.0, std::f32::consts::FRAC_PI_3]),
        ),
        "extrusion_helix" => Geometry::from(Extrusion::new(
            helix_axis(2.0, 64),
            circle_section(0.15, s),
        )),
        _ => return None,
    })
}

/// The curve cases, mirroring `CURVE_CASES` in `upstream_measure.py`.
///
/// A curve is not a mesh: upstream's discretizer reduces it to a `Polyline`, so
/// there is no area or face count to compare. What there is instead is the only
/// thing that matters about an evaluator — where it says the curve is.
enum SampledCurve {
    ThreeD(Curve3D),
    TwoD(Curve2D),
}

fn build_curve(case: &str) -> Option<SampledCurve> {
    let three_d = |curve: Curve3D| Some(SampledCurve::ThreeD(curve));
    let two_d = |curve: Curve2D| Some(SampledCurve::TwoD(curve));
    match case {
        "bezier_curve_cubic" => three_d(Curve3D::from(
            BezierCurve::new(vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 4.0, -2.0),
                Point3::new(3.0, -1.0, 2.0),
                Point3::new(5.0, 2.0, 0.0),
            ])
            .with_stride(CURVE_STRIDE),
        )),
        "bezier_curve_rational" => three_d(Curve3D::from(
            BezierCurve::rational(vec![
                Vec4::new(1.0, 0.0, 0.0, 1.0),
                Vec4::new(1.0, 1.0, 0.0, 0.5),
                Vec4::new(0.0, 1.0, 1.0, 2.0),
                Vec4::new(-1.0, 0.0, 1.0, 1.0),
            ])
            .with_stride(CURVE_STRIDE),
        )),
        "nurbs_curve_cubic" => three_d(Curve3D::from(
            NurbsCurve::new(
                vec![
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(1.0, 2.0, 0.0),
                    Point3::new(2.0, -1.0, 1.0),
                    Point3::new(3.0, 1.0, 2.0),
                    Point3::new(4.0, 0.0, 0.0),
                    Point3::new(5.0, 2.0, 1.0),
                ],
                3,
            )
            .with_stride(CURVE_STRIDE),
        )),
        "nurbs_curve_knots" => three_d(Curve3D::from(
            NurbsCurve::with_knots(
                vec![
                    Vec4::new(0.0, 0.0, 0.0, 1.0),
                    Vec4::new(1.0, 3.0, 0.0, 1.0),
                    Vec4::new(2.0, 0.0, 2.0, 1.0),
                    Vec4::new(4.0, 1.0, 0.0, 1.0),
                    Vec4::new(5.0, -1.0, 1.0, 1.0),
                ],
                2,
                vec![0.0, 0.0, 0.0, 0.3, 0.75, 1.0, 1.0, 1.0],
            )
            .expect("a valid knot vector")
            .with_stride(CURVE_STRIDE),
        )),
        "bezier_curve_2d" => two_d(Curve2D::from(
            BezierCurve2D::new(vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 2.0),
                Point2::new(3.0, -1.0),
                Point2::new(4.0, 0.0),
            ])
            .with_stride(CURVE_STRIDE),
        )),
        "nurbs_curve_2d_circle" => {
            two_d(Curve2D::from(NurbsCurve2D::circle(1.0).with_stride(CURVE_STRIDE)))
        }
        _ => None,
    }
}

impl SampledCurve {
    fn first_knot(&self) -> Real {
        match self {
            SampledCurve::ThreeD(c) => c.first_knot(),
            SampledCurve::TwoD(c) => c.first_knot(),
        }
    }

    fn last_knot(&self) -> Real {
        match self {
            SampledCurve::ThreeD(c) => c.last_knot(),
            SampledCurve::TwoD(c) => c.last_knot(),
        }
    }

    /// The point at `u`, always as three coordinates; a 2D curve sits at
    /// `z = 0`, which is how upstream's discretizer embeds it too.
    fn eval(&self, u: Real) -> [Real; 3] {
        match self {
            SampledCurve::ThreeD(c) => {
                let p = c.eval(u).expect("evaluation");
                [p.x, p.y, p.z]
            }
            SampledCurve::TwoD(c) => {
                let p = c.eval(u).expect("evaluation");
                [p.x, p.y, 0.0]
            }
        }
    }

    fn tangent(&self, u: Real) -> [Real; 3] {
        match self {
            SampledCurve::ThreeD(c) => {
                let t = c.tangent(u).expect("tangent");
                [t.x, t.y, t.z]
            }
            SampledCurve::TwoD(c) => {
                let t = c.tangent(u).expect("tangent");
                [t.x, t.y, 0.0]
            }
        }
    }

    fn discretize(&self) -> Vec<[Real; 3]> {
        match self {
            SampledCurve::ThreeD(c) => c
                .discretize(CURVE_STRIDE)
                .expect("discretisation")
                .into_iter()
                .map(|p| [p.x, p.y, p.z])
                .collect(),
            SampledCurve::TwoD(c) => c
                .discretize(CURVE_STRIDE)
                .expect("discretisation")
                .into_iter()
                .map(|p| [p.x, p.y, 0.0])
                .collect(),
        }
    }

    fn length(&self) -> Real {
        match self {
            SampledCurve::ThreeD(c) => c.length(CURVE_STRIDE).expect("length"),
            SampledCurve::TwoD(c) => c.length(CURVE_STRIDE).expect("length"),
        }
    }
}

// --- Reading the reference --------------------------------------------------

/// One upstream measurement, parsed from the committed JSON.
#[derive(Debug, Clone)]
struct Reference {
    points: usize,
    faces: usize,
    triangles: usize,
    degenerate_faces: usize,
    inward_faces: usize,
    solid: bool,
    ccw: bool,
    bbox_min: [Real; 3],
    bbox_max: [Real; 3],
    area: Real,
    volume: Real,
    area_measures_mesh: bool,
    volume_measures_mesh: bool,
}

/// A deliberately small JSON reader.
///
/// The crate has no runtime JSON dependency and this file is generated by our
/// own script in a fixed shape, so pulling `serde_json` into the dev
/// dependencies to read eleven scalars per case is not worth it. The parser is
/// strict: a key it cannot find is a panic naming the key, not a default.
mod mini_json {
    pub fn object_body<'a>(text: &'a str, key: &str) -> Option<&'a str> {
        let needle = format!("\"{key}\"");
        let start = text.find(&needle)? + needle.len();
        let open = start + text[start..].find('{')?;
        let mut depth = 0usize;
        for (offset, byte) in text[open..].bytes().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&text[open + 1..open + offset]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The top-level `"name": { … }` entries of an object body.
    pub fn entries(body: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let bytes = body.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != b'"' {
                i += 1;
                continue;
            }
            let key_start = i + 1;
            let Some(key_len) = body[key_start..].find('"') else {
                break;
            };
            let key = &body[key_start..key_start + key_len];
            let after = key_start + key_len + 1;
            let Some(brace) = body[after..].find('{') else {
                break;
            };
            let open = after + brace;
            let mut depth = 0usize;
            let mut end = open;
            for (offset, byte) in body[open..].bytes().enumerate() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = open + offset;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push((key.to_string(), body[open + 1..end].to_string()));
            i = end + 1;
        }
        out
    }

    pub fn scalar<'a>(body: &'a str, key: &str) -> &'a str {
        let needle = format!("\"{key}\"");
        let start = body
            .find(&needle)
            .unwrap_or_else(|| panic!("reference is missing key {key:?}"))
            + needle.len();
        let rest = body[start..].trim_start();
        let rest = rest
            .strip_prefix(':')
            .unwrap_or_else(|| panic!("key {key:?} is not followed by a colon"))
            .trim_start();
        let end = rest
            .find([',', '\n', '}'])
            .unwrap_or(rest.len());
        rest[..end].trim().trim_matches('"')
    }

    /// A flat array of numbers, however it is wrapped across lines. The
    /// generator writes point lists flattened precisely so this stays a
    /// bracket scan rather than a JSON parser.
    pub fn numbers(body: &str, key: &str) -> Vec<f32> {
        let needle = format!("\"{key}\"");
        let start = body
            .find(&needle)
            .unwrap_or_else(|| panic!("reference is missing array {key:?}"))
            + needle.len();
        let open = start + body[start..].find('[').expect("array open");
        let close = open + body[open..].find(']').expect("array close");
        body[open + 1..close]
            .split(',')
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                value
                    .trim()
                    .parse::<f32>()
                    .unwrap_or_else(|e| panic!("bad number in {key}: {value:?}: {e}"))
            })
            .collect()
    }

    pub fn triple(body: &str, key: &str) -> [f32; 3] {
        let needle = format!("\"{key}\"");
        let start = body.find(&needle).expect("triple key") + needle.len();
        let open = start + body[start..].find('[').expect("array open");
        let close = open + body[open..].find(']').expect("array close");
        let mut values = body[open + 1..close].split(',').map(|v| {
            v.trim()
                .parse::<f32>()
                .unwrap_or_else(|e| panic!("bad number in {key}: {v:?}: {e}"))
        });
        [
            values.next().expect("x"),
            values.next().expect("y"),
            values.next().expect("z"),
        ]
    }
}

fn references() -> BTreeMap<String, Reference> {
    let cases = mini_json::object_body(REFERENCE, "cases").expect("reference has a `cases` object");
    assert!(
        !REFERENCE.contains("\"failures\""),
        "the reference was generated with failing cases; regenerate it and fix them"
    );

    mini_json::entries(cases)
        .into_iter()
        .map(|(name, body)| {
            let number = |key: &str| -> Real {
                mini_json::scalar(&body, key)
                    .parse()
                    .unwrap_or_else(|e| panic!("{name}.{key}: {e}"))
            };
            let count = |key: &str| -> usize {
                mini_json::scalar(&body, key)
                    .parse()
                    .unwrap_or_else(|e| panic!("{name}.{key}: {e}"))
            };
            let flag = |key: &str| -> bool { mini_json::scalar(&body, key) == "true" };
            let reference = Reference {
                points: count("points"),
                faces: count("faces"),
                triangles: count("triangles"),
                degenerate_faces: count("degenerate_faces"),
                inward_faces: count("inward_faces"),
                solid: flag("solid"),
                ccw: flag("ccw"),
                bbox_min: mini_json::triple(&body, "bbox_min"),
                bbox_max: mini_json::triple(&body, "bbox_max"),
                area: number("area"),
                volume: number("volume"),
                area_measures_mesh: flag("area_measures_mesh"),
                volume_measures_mesh: flag("volume_measures_mesh"),
            };
            (name, reference)
        })
        .collect()
}

/// One upstream curve measurement.
#[derive(Debug, Clone)]
struct CurveReference {
    first_knot: Real,
    last_knot: Real,
    stride: u32,
    dimensions: usize,
    samples: Vec<[Real; 3]>,
    tangents: Vec<[Real; 3]>,
    tangent_is_comparable: bool,
    tangent_endpoints_comparable: bool,
    discretized: Vec<[Real; 3]>,
    length: Real,
}

fn curve_references() -> BTreeMap<String, CurveReference> {
    let curves =
        mini_json::object_body(REFERENCE, "curves").expect("reference has a `curves` object");

    mini_json::entries(curves)
        .into_iter()
        .map(|(name, body)| {
            let dimensions: usize = mini_json::scalar(&body, "dimensions").parse().expect("dims");
            // A 2D curve's points are written as pairs; pad them to triples so
            // both kinds compare through one code path, exactly as upstream's
            // discretizer embeds a 2D curve at z = 0.
            let group = |key: &str| -> Vec<[Real; 3]> {
                mini_json::numbers(&body, key)
                    .chunks(dimensions)
                    .map(|c| [c[0], c[1], if dimensions == 3 { c[2] } else { 0.0 }])
                    .collect()
            };
            let reference = CurveReference {
                first_knot: mini_json::scalar(&body, "first_knot").parse().expect("first"),
                last_knot: mini_json::scalar(&body, "last_knot").parse().expect("last"),
                stride: mini_json::scalar(&body, "stride").parse().expect("stride"),
                dimensions,
                samples: group("samples"),
                tangents: group("tangents"),
                tangent_is_comparable: mini_json::scalar(&body, "tangent_is_comparable") == "true",
                tangent_endpoints_comparable: mini_json::scalar(
                    &body,
                    "tangent_endpoints_comparable",
                ) == "true",
                // The discretizer always writes three coordinates.
                discretized: mini_json::numbers(&body, "discretized")
                    .chunks(3)
                    .map(|c| [c[0], c[1], c[2]])
                    .collect(),
                length: mini_json::scalar(&body, "length").parse().expect("length"),
            };
            (name, reference)
        })
        .collect()
}

/// `"cone_solid@32"` → `("cone_solid", 32)`.
fn split_key(key: &str) -> (&str, u8) {
    let (case, slices) = key.rsplit_once('@').expect("keys are `case@slices`");
    (case, slices.parse().expect("slice count"))
}

fn relative_error(actual: Real, expected: Real) -> Real {
    if expected.abs() <= 1e-9 {
        return actual.abs();
    }
    ((actual - expected) / expected).abs()
}

fn norm(v: [Real; 3]) -> Real {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn distance(a: [Real; 3], b: [Real; 3]) -> Real {
    norm([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
}

/// A vector normalised, for comparing directions rather than magnitudes.
fn direction(v: [Real; 3]) -> [Real; 3] {
    let n = norm(v);
    if n <= 1e-9 {
        return v;
    }
    [v[0] / n, v[1] / n, v[2] / n]
}

// --- The tests --------------------------------------------------------------

/// The reference must actually contain something, or every test below passes
/// vacuously.
#[test]
fn the_reference_is_present_and_populated() {
    let references = references();
    assert!(
        references.len() >= 30,
        "expected the full case matrix, got {} entries",
        references.len()
    );
    assert!(REFERENCE.contains("\"plantgl_version\""));
}

/// Every case in the reference is built here, and every case built here is in
/// the reference. Without this, a typo silently drops a primitive from the
/// comparison.
#[test]
fn every_reference_case_is_covered() {
    let references = references();
    let mut missing = Vec::new();
    for key in references.keys() {
        let (case, slices) = split_key(key);
        if build(case, slices).is_none() {
            missing.push(key.clone());
        }
    }
    assert!(missing.is_empty(), "no Rust builder for: {missing:?}");

    // And the coverage the issues ask for: every T8.4 primitive and every T8.7
    // sweep, at two slice counts each.
    for required in [
        "box",
        "sphere",
        "cone",
        "cylinder",
        "frustum",
        "disc",
        "paraboloid",
        "revolution",
        "elevation_grid",
        "bezier_patch",
        "nurbs_patch",
        "extrusion",
    ] {
        let count = references
            .keys()
            .filter(|k| k.starts_with(required))
            .count();
        assert!(
            count >= 2,
            "{required} appears in {count} reference cases, expected at least 2 (two slice counts)"
        );
    }
}

/// Mesh topology must match upstream exactly.
#[test]
fn topology_matches_upstream() {
    let mut failures = Vec::new();

    for (key, reference) in references() {
        let (case, slices) = split_key(&key);
        let geometry = build(case, slices).expect("covered");
        let ctx = DiscretizeCtx {
            slices,
            stacks: slices,
            curve_samples: 64,
        };
        let mesh = discretize_with(&geometry, ctx).expect("discretisation");

        let poles = pole_collapse(case);
        // Upstream duplicates each pole once per slice; the port keeps one.
        let expected_points = reference.points - poles * (slices as usize - 1);
        // Upstream's extra faces at a pole are exactly the degenerate ones it
        // reported, so the expectation comes from its own measurement.
        let expected_faces = if poles > 0 {
            reference.faces - reference.degenerate_faces
        } else {
            reference.faces
        };

        let mut note = |what: &str, ours: String, theirs: String| {
            failures.push(format!("{key}: {what} ours={ours} upstream={theirs}"));
        };

        if mesh.points().len() != expected_points {
            note(
                "points",
                mesh.points().len().to_string(),
                format!("{} (expected {expected_points})", reference.points),
            );
        }
        if mesh.face_count() != expected_faces {
            note(
                "faces",
                mesh.face_count().to_string(),
                format!("{} (expected {expected_faces})", reference.faces),
            );
        }
        if mesh.is_solid() != reference.solid {
            note("solid", mesh.is_solid().to_string(), reference.solid.to_string());
        }
        let ccw = match &mesh {
            Explicit::TriangleSet(m) => m.model.ccw,
            Explicit::QuadSet(m) => m.model.ccw,
            Explicit::FaceSet(m) => m.model.ccw,
            _ => true,
        };
        if ccw != reference.ccw {
            note("ccw", ccw.to_string(), reference.ccw.to_string());
        }
    }

    assert!(
        failures.is_empty(),
        "topology diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn triangle_counts_match_upstream() {
    let mut failures = Vec::new();

    for (key, reference) in references() {
        let (case, slices) = split_key(&key);
        let geometry = build(case, slices).expect("covered");
        let ctx = DiscretizeCtx {
            slices,
            stacks: slices,
            curve_samples: 64,
        };
        let mesh = discretize_with(&geometry, ctx).expect("discretisation");
        let ours = tessellate(&mesh).expect("tessellation").face_count();

        let poles = pole_collapse(case);
        let expected = if poles > 0 {
            reference.triangles - reference.degenerate_faces
        } else {
            reference.triangles
        };

        if ours != expected {
            failures.push(format!(
                "{key}: triangles ours={ours} upstream={} (expected {expected})",
                reference.triangles
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "triangle counts diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn bounding_boxes_match_upstream() {
    let mut failures = Vec::new();

    for (key, reference) in references() {
        let (case, slices) = split_key(&key);
        let geometry = build(case, slices).expect("covered");
        let mut computer = plantgl::algo::bbox::BBoxComputer::with_discretizer(
            plantgl::algo::discretize::Discretizer::new(DiscretizeCtx {
                slices,
                stacks: slices,
                curve_samples: 64,
            })
            .with_tex_coords(false),
        );
        let bbox = computer
            .compute(&geometry.into_ref())
            .expect("bbox")
            .expect("non-empty");

        for (axis, (ours, theirs)) in [
            (bbox.lower_left.x, reference.bbox_min[0]),
            (bbox.lower_left.y, reference.bbox_min[1]),
            (bbox.lower_left.z, reference.bbox_min[2]),
            (bbox.upper_right.x, reference.bbox_max[0]),
            (bbox.upper_right.y, reference.bbox_max[1]),
            (bbox.upper_right.z, reference.bbox_max[2]),
        ]
        .iter()
        .enumerate()
        {
            // A helix sweep's rings are rotated relative to upstream's, so its
            // box may differ by the cross-section's sagitta and no more.
            let tolerance =
                BBOX_TOLERANCE + frame_divergence_bound(case, slices).unwrap_or(0.0);
            if (ours - theirs).abs() > tolerance {
                failures.push(format!(
                    "{key}: bbox component {axis} ours={ours} upstream={theirs} \
                     (tolerance {tolerance})"
                ));
            }
        }
        let _ = bounding_box; // the convenience wrapper, exercised elsewhere
    }

    assert!(
        failures.is_empty(),
        "bounding boxes diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

/// ⭐ The acceptance criterion: every primitive matches upstream's numbers
/// within tolerance.
#[test]
fn surface_areas_match_upstream() {
    let mut failures = Vec::new();

    for (key, reference) in references() {
        let (case, slices) = split_key(&key);
        let geometry = build(case, slices).expect("covered");
        let ctx = DiscretizeCtx {
            slices,
            stacks: slices,
            curve_samples: 64,
        };
        let mesh = discretize_with(&geometry, ctx).expect("discretisation");
        let ours = surface_area(&mesh).expect("area");

        // Where upstream's formula is wrong, the correct closed form is the
        // thing to converge to — not upstream's number.
        let defect = upstream_area_defect(case);
        let expected = defect.unwrap_or(reference.area);
        let error = relative_error(ours, expected);

        let (tolerance, kind) = if frame_divergence_bound(case, slices).is_some() {
            (
                FRAME_DIVERGENCE_AREA_TOLERANCE,
                "mesh-measured, rings rotated by the frame divergence",
            )
        } else if reference.area_measures_mesh {
            // Both sides measured the same mesh: they must agree, not merely
            // be close.
            (MESH_TOLERANCE, "mesh-measured")
        } else if slices >= 32 {
            (
                CONVERGENCE_TOLERANCE,
                if defect.is_some() {
                    "closed form, fine (upstream's formula is wrong)"
                } else {
                    "analytic, fine"
                },
            )
        } else {
            (
                COARSE_AREA_TOLERANCE,
                if defect.is_some() {
                    "closed form, coarse (upstream's formula is wrong)"
                } else {
                    "analytic, coarse"
                },
            )
        };

        if error > tolerance {
            failures.push(format!(
                "{key} ({kind}): area ours={ours} expected={expected} \
                 relative error {error:.5} > {tolerance}"
            ));
        }
        // A discretisation is inscribed, so it may approach the ideal from
        // below but never exceed it.
        if !reference.area_measures_mesh && ours > expected * (1.0 + MESH_TOLERANCE) {
            failures.push(format!("{key}: area ours={ours} exceeds the ideal {expected}"));
        }
    }

    assert!(
        failures.is_empty(),
        "surface areas diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn volumes_match_upstream() {
    let mut failures = Vec::new();

    for (key, reference) in references() {
        let (case, slices) = split_key(&key);
        let geometry = build(case, slices).expect("covered");
        let ctx = DiscretizeCtx {
            slices,
            stacks: slices,
            curve_samples: 64,
        };
        let mesh = discretize_with(&geometry, ctx).expect("discretisation");

        if !reference.solid {
            // Upstream reports 0 for a non-solid mesh; the port reports an
            // error, which is the documented divergence. Check that, rather
            // than skipping.
            assert!(
                volume(&mesh).is_err(),
                "{key}: upstream reports no volume for a non-solid mesh, so the port must error"
            );
            continue;
        }

        let ours = volume(&mesh).expect("volume");
        let error = relative_error(ours, reference.volume);

        let (tolerance, kind) = if reference.volume_measures_mesh {
            (MESH_TOLERANCE, "mesh-measured")
        } else if slices >= 32 {
            (CONVERGENCE_TOLERANCE, "analytic, fine")
        } else {
            (COARSE_VOLUME_TOLERANCE, "analytic, coarse")
        };

        if error > tolerance {
            failures.push(format!(
                "{key} ({kind}): volume ours={ours} upstream={} relative error {error:.5} > {tolerance}",
                reference.volume
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "volumes diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

/// The pole-collapse divergence, asserted rather than excused.
///
/// Upstream's zero-area triangles at a swept pole are real and counted in the
/// reference; the port must produce exactly that many fewer faces, and the
/// same surface area, because a zero-area triangle contributes nothing.
#[test]
fn the_pole_collapse_divergence_is_exactly_what_is_claimed() {
    let references = references();
    let mut checked = 0;

    for (key, reference) in &references {
        let (case, slices) = split_key(key);
        if pole_collapse(case) == 0 {
            continue;
        }
        checked += 1;

        // Upstream really does emit one degenerate face per slice per pole.
        assert_eq!(
            reference.degenerate_faces,
            pole_collapse(case) * slices as usize,
            "{key}: upstream's degenerate face count is not `poles × slices`"
        );

        let geometry = build(case, slices).expect("covered");
        let mesh = discretize_with(
            &geometry,
            DiscretizeCtx {
                slices,
                stacks: slices,
                curve_samples: 64,
            },
        )
        .expect("discretisation");

        // Exactly the degenerate faces are gone, and nothing else.
        assert_eq!(
            mesh.face_count(),
            reference.faces - reference.degenerate_faces,
            "{key}: the port should drop exactly upstream's degenerate faces"
        );

        // Dropping zero-area triangles cannot change the area.
        let error = relative_error(surface_area(&mesh).unwrap(), reference.area);
        assert!(
            error < MESH_TOLERANCE,
            "{key}: collapsing the poles changed the surface area (relative error {error})"
        );

        // And the port's mesh has no degenerate faces left at all.
        let Explicit::TriangleSet(triangles) = &mesh else {
            panic!("{key} discretises to triangles");
        };
        for i in 0..triangles.face_count() {
            let area = plantgl::algo::measure::triangle_area(
                triangles.face_point_at(i, 0).unwrap(),
                triangles.face_point_at(i, 1).unwrap(),
                triangles.face_point_at(i, 2).unwrap(),
            );
            assert!(area > 1e-9, "{key}: triangle {i} is still degenerate");
        }
    }

    assert!(checked > 0, "no pole-collapse case was actually checked");
}

/// The known upstream area defects, pinned.
///
/// Each entry in [`upstream_area_defect`] claims that upstream's number is
/// wrong by a specific amount. If an upstream rebase fixes one, this fails and
/// the entry should be deleted — without it, a corrected upstream would be
/// silently compared against a redundant hardcoded constant forever.
#[test]
fn known_upstream_area_defects_are_still_present() {
    let references = references();
    let mut checked = 0;

    for (key, reference) in &references {
        let (case, _) = split_key(key);
        let Some(correct) = upstream_area_defect(case) else {
            continue;
        };
        checked += 1;
        assert!(
            relative_error(reference.area, correct) > CONVERGENCE_TOLERANCE,
            "{key}: upstream now reports {} against a correct {correct} — the defect \
             looks fixed, so remove this case from `upstream_area_defect`",
            reference.area
        );
    }
    assert!(checked > 0, "no defect case was actually checked");

    // The specific diagnosis, so the entries above are falsifiable rather than
    // just "upstream differs".
    let disc_r3 = references["disc_r3@32"].area;
    let circumference = std::f32::consts::TAU * 3.0;
    assert!(
        (disc_r3 - circumference).abs() < 1e-3,
        "upstream's radius-3 disc area {disc_r3} should be the circumference {circumference}"
    );

    let frustum = references["frustum_solid@32"].area;
    let (r, q, h) = (2.0f32, 1.0f32, 3.0f32);
    let slant = ((r - q) * (r - q) + h * h).sqrt();
    let with_wrong_caps = std::f32::consts::PI * ((r + q) * slant + r + q);
    assert!(
        (frustum - with_wrong_caps).abs() < 1e-3,
        "upstream's solid frustum area {frustum} should be the lateral surface plus \
         π(r + q) = {with_wrong_caps}"
    );

    // And the case where the defect is provably invisible: at r = 1, πr and
    // πr² coincide, so upstream's zero-taper frustum is accidentally right.
    let cone = references["frustum_cone@32"].area;
    let correct_cone = std::f32::consts::PI * (1.0 * (1.0f32 + 4.0).sqrt() + 1.0);
    assert!(
        relative_error(cone, correct_cone) < 1e-4,
        "at r = 1 upstream's frustum area {cone} should coincide with the correct {correct_cone}"
    );
}

// --- Curves (T8.6) ----------------------------------------------------------

/// Every curve in the reference is built here, and the T8.6 deliverables are
/// each represented.
#[test]
fn every_reference_curve_is_covered() {
    let curves = curve_references();
    assert!(!curves.is_empty(), "the reference has no curve cases");

    let missing: Vec<&String> = curves
        .keys()
        .filter(|name| build_curve(name).is_none())
        .collect();
    assert!(missing.is_empty(), "no Rust builder for: {missing:?}");

    for required in [
        "bezier_curve_cubic",
        "bezier_curve_rational",
        "nurbs_curve_cubic",
        "nurbs_curve_knots",
        "bezier_curve_2d",
        "nurbs_curve_2d_circle",
    ] {
        assert!(curves.contains_key(required), "{required} is not covered");
    }

    // The knot ranges have to agree before anything sampled over them can.
    for (name, reference) in &curves {
        let curve = build_curve(name).expect("covered");
        assert!(
            (curve.first_knot() - reference.first_knot).abs() < 1e-6
                && (curve.last_knot() - reference.last_knot).abs() < 1e-6,
            "{name}: knot range ours=[{}, {}] upstream=[{}, {}]",
            curve.first_knot(),
            curve.last_knot(),
            reference.first_knot,
            reference.last_knot
        );
        assert_eq!(reference.stride, CURVE_STRIDE, "{name}: stride");

        // A 2D curve is embedded at z = 0 by both sides, which is what lets
        // every comparison below run through one three-coordinate path.
        assert!(matches!(reference.dimensions, 2 | 3), "{name}: dimensions");
        if reference.dimensions == 2 {
            assert!(
                reference.discretized.iter().all(|p| p[2].abs() < 1e-9),
                "{name}: upstream should embed a 2D curve at z = 0"
            );
            assert!(
                curve.discretize().iter().all(|p| p[2].abs() < 1e-9),
                "{name}: the port should embed a 2D curve at z = 0"
            );
        }
    }
}

/// ⭐ The T8.6 acceptance criterion: curve samples match upstream's within
/// tolerance.
#[test]
fn curve_samples_match_upstream() {
    let mut failures = Vec::new();

    for (name, reference) in curve_references() {
        let curve = build_curve(&name).expect("covered");
        let count = reference.samples.len();
        assert!(count > 2, "{name}: too few samples to mean anything");

        for (i, theirs) in reference.samples.iter().enumerate() {
            let u = reference.first_knot
                + (reference.last_knot - reference.first_knot) * i as Real / (count - 1) as Real;
            let ours = curve.eval(u);
            let error = distance(ours, *theirs);
            if error > CURVE_TOLERANCE {
                failures.push(format!(
                    "{name} at u={u}: ours={ours:?} upstream={theirs:?} error {error}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "curve samples diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

/// The discretisation itself — the polyline a `Discretizer` turns each curve
/// into, which is what every downstream algorithm actually consumes.
#[test]
fn curve_discretisations_match_upstream() {
    let mut failures = Vec::new();

    for (name, reference) in curve_references() {
        let curve = build_curve(&name).expect("covered");
        let ours = curve.discretize();
        if ours.len() != reference.discretized.len() {
            failures.push(format!(
                "{name}: {} points against upstream's {}",
                ours.len(),
                reference.discretized.len()
            ));
            continue;
        }
        for (i, (ours, theirs)) in ours.iter().zip(&reference.discretized).enumerate() {
            let error = distance(*ours, *theirs);
            if error > CURVE_TOLERANCE {
                failures.push(format!("{name}: point {i} is {error} away from upstream's"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "curve discretisations diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

/// Tangents, where upstream's are trustworthy.
///
/// `NurbsCurve::getTangentAt` goes through `deriveAt`, which applies the
/// quotient rule and is right for rational curves; the port must agree with it.
/// `BezierCurve::getTangentAt` does not, and the case that exposes it is pinned
/// separately below.
#[test]
fn curve_tangents_match_upstream() {
    let mut failures = Vec::new();
    let mut checked = 0;

    for (name, reference) in curve_references() {
        if !reference.tangent_is_comparable {
            continue;
        }
        checked += 1;
        let curve = build_curve(&name).expect("covered");
        let count = reference.tangents.len();

        for (i, theirs) in reference.tangents.iter().enumerate() {
            if !reference.tangent_endpoints_comparable && (i == 0 || i == count - 1) {
                continue;
            }
            let u = reference.first_knot
                + (reference.last_knot - reference.first_knot) * i as Real / (count - 1) as Real;
            let ours = curve.tangent(u);
            // Compared relative to the tangent's own magnitude: a degree-3
            // curve's derivative is three times the size of its coordinates,
            // and it is the direction and scale together that must agree.
            let scale = norm(*theirs).max(1.0);
            let error = distance(ours, *theirs) / scale;
            if error > CURVE_TOLERANCE {
                failures.push(format!(
                    "{name} at u={u}: tangent ours={ours:?} upstream={theirs:?} \
                     relative error {error}"
                ));
            }
        }
    }

    assert!(checked >= 4, "only {checked} curves had comparable tangents");
    assert!(
        failures.is_empty(),
        "curve tangents diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

/// **`BezierCurve::getTangentAt` is wrong for a rational curve, and the port
/// does not reproduce it.**
///
/// Upstream differences the stored control points — which are Cartesian points
/// plus a weight, not homogeneous points — and calls `project()` on the result,
/// dividing by a *difference of weights*. For an all-weight-1 curve that
/// difference is zero and the guard returns the correct answer, which is why
/// every other Bézier case compares as normal. For this one it is not a tangent
/// at all.
///
/// Both halves are asserted, so neither can rot: the port's tangent matches a
/// central difference of *upstream's own sampled points* — upstream's points
/// being correct — and upstream's tangent does not. If a rebase fixes
/// `getTangentAt`, the second assertion fails and this test should be replaced
/// by adding the case back to the comparable set.
#[test]
fn the_rational_bezier_tangent_defect_is_still_present() {
    let name = "bezier_curve_rational";
    let reference = &curve_references()[name];
    assert!(
        !reference.tangent_is_comparable,
        "{name} is marked comparable, so this test has nothing to pin"
    );

    let curve = build_curve(name).expect("covered");
    let count = reference.samples.len();
    let step = (reference.last_knot - reference.first_knot) / (count - 1) as Real;

    let mut ours_agrees = 0;
    let mut upstream_disagrees = 0;
    // Interior samples only: a central difference needs a neighbour each side.
    for i in 1..count - 1 {
        let u = reference.first_knot + step * i as Real;
        let numeric = direction([
            (reference.samples[i + 1][0] - reference.samples[i - 1][0]) / (2.0 * step),
            (reference.samples[i + 1][1] - reference.samples[i - 1][1]) / (2.0 * step),
            (reference.samples[i + 1][2] - reference.samples[i - 1][2]) / (2.0 * step),
        ]);

        // The port's tangent is the real derivative, so it points where the
        // curve is actually going. The yardstick is a central difference over
        // a thirty-second of the curve, whose own truncation error is a few
        // parts in a thousand — hence the threshold, which is still two orders
        // below how far upstream's answer lands from it.
        let ours = direction(curve.tangent(u));
        assert!(
            distance(ours, numeric) < 5e-3,
            "the port's tangent at u={u} is {ours:?}, but the curve goes {numeric:?}"
        );
        ours_agrees += 1;

        if distance(direction(reference.tangents[i]), numeric) > 5e-2 {
            upstream_disagrees += 1;
        }
    }

    assert!(ours_agrees > 20, "too few samples checked");
    assert!(
        upstream_disagrees > ours_agrees / 2,
        "upstream's rational Bézier tangent now agrees with the curve's own \
         direction at all but {} of {ours_agrees} samples — the defect looks \
         fixed, so move `bezier_curve_rational` into the comparable set",
        upstream_disagrees
    );
}

/// **`BezierCurve::getTangentAt` is also wrong at both of its end points**, and
/// differently wrong at each.
///
/// Its interior branch is the true derivative, `n · Σ (P[i+1] - P[i]) B[i,n-1]`.
/// Its two special cases are not:
///
/// - at `u = 0` it returns `P1 - P0` **normalised**, a unit vector where the
///   derivative has magnitude `n · |P1 - P0|`;
/// - at `u = 1` it returns `P[n] - P[n-1]` **unscaled**, short by the factor
///   `n`.
///
/// So upstream's own tangent field is discontinuous at both ends of every
/// Bézier curve, including the all-weight-1 ones. The port returns the
/// derivative throughout, which is the limit of upstream's interior branch —
/// asserted here from upstream's own recorded numbers, so the diagnosis is
/// falsifiable and not just "we differ".
#[test]
fn the_bezier_endpoint_tangent_defects_are_still_present() {
    let curves = curve_references();
    let mut checked = 0;

    for (name, reference) in &curves {
        // Weight-1 Bézier curves only: a *rational* one goes through
        // `project()` at both ends as well, so its endpoint values are wrong in
        // the compounded way `the_rational_bezier_tangent_defect_is_still_present`
        // pins instead, and the two clean statements below do not hold there.
        if reference.tangent_endpoints_comparable || !reference.tangent_is_comparable {
            continue;
        }
        checked += 1;
        let curve = build_curve(name).expect("covered");
        let last = reference.tangents.len() - 1;

        // At u = 0 upstream's vector is a unit one where the derivative is not.
        let theirs_start = reference.tangents[0];
        let ours_start = curve.tangent(reference.first_knot);
        assert!(
            (norm(theirs_start) - 1.0).abs() < 1e-5,
            "{name}: upstream's tangent at u = 0 is {theirs_start:?}, no longer \
             a unit vector — the defect looks fixed"
        );
        assert!(
            norm(ours_start) > 1.0 + 1e-3,
            "{name}: the port's tangent at u = 0 should be the derivative, not \
             a unit vector"
        );
        // Same direction, though: only the magnitude is wrong.
        assert!(
            distance(direction(ours_start), direction(theirs_start)) < 1e-5,
            "{name}: the port and upstream disagree on the *direction* at u = 0"
        );

        // At u = 1 upstream is short by exactly the degree.
        let theirs_end = reference.tangents[last];
        let ours_end = curve.tangent(reference.last_knot);
        let ratio = norm(ours_end) / norm(theirs_end);
        assert!(
            (ratio - 3.0).abs() < 1e-3,
            "{name}: the port's end tangent is {ratio}x upstream's, expected the \
             curve's degree (3) — upstream's missing factor looks fixed"
        );
        assert!(
            distance(direction(ours_end), direction(theirs_end)) < 1e-5,
            "{name}: the port and upstream disagree on the *direction* at u = 1"
        );

        // And the port's end points are the limit of upstream's own interior
        // branch, which is the positive claim behind ignoring its end points.
        let step = (reference.last_knot - reference.first_knot) / last as Real;
        for u in [reference.first_knot, reference.last_knot] {
            let inward = if u == reference.first_knot { step } else { -step };
            let interior = if u == reference.first_knot {
                reference.tangents[1]
            } else {
                reference.tangents[last - 1]
            };
            let extrapolated = curve.tangent(u + inward);
            assert!(
                distance(extrapolated, interior) / norm(interior).max(1.0) < CURVE_TOLERANCE,
                "{name}: one step in from u = {u} the port gives {extrapolated:?} \
                 against upstream's interior {interior:?}"
            );
        }
    }

    assert!(checked >= 2, "only {checked} Bézier curves were pinned");
}

/// Curve length, which is what a swept surface's texture coordinates and a
/// turtle's step both run on.
#[test]
fn curve_lengths_match_upstream() {
    let mut failures = Vec::new();

    for (name, reference) in curve_references() {
        let curve = build_curve(&name).expect("covered");
        let ours = curve.length();
        // Both sides measure the same polyline through the same samples, so
        // this is a mesh-to-mesh comparison and not a convergence one.
        let error = relative_error(ours, reference.length);
        if error > MESH_TOLERANCE {
            failures.push(format!(
                "{name}: length ours={ours} upstream={} relative error {error}",
                reference.length
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "curve lengths diverged from upstream:\n  {}",
        failures.join("\n  ")
    );
}

/// The frame divergence, asserted rather than excused.
///
/// Where the axis is straight the two frame chains provably coincide, so the
/// meshes must be identical vertex for vertex — that is what the strict
/// comparisons in the tests above already say for `extrusion_straight` and its
/// siblings. Where the axis has torsion they differ, and this says by how much:
/// each ring is *rotated*, so every one of the port's vertices still lies on
/// upstream's ring, within the tolerance that the ring is a polygon at all.
#[test]
fn the_frame_divergence_is_only_a_rotation_of_each_ring() {
    let references = references();
    let mut checked = 0;

    for (key, reference) in &references {
        let (case, slices) = split_key(key);
        let Some(bound) = frame_divergence_bound(case, slices) else {
            continue;
        };
        checked += 1;

        let mesh = discretize_with(
            &build(case, slices).expect("covered"),
            DiscretizeCtx {
                slices,
                stacks: slices,
                curve_samples: 64,
            },
        )
        .expect("discretisation");

        // Same topology: the divergence moves vertices, it does not add or
        // drop any.
        assert_eq!(mesh.points().len(), reference.points, "{key}: point count");
        assert_eq!(mesh.face_count(), reference.faces, "{key}: face count");

        // The helix axis is a circle of radius 1 in x and y, and the sweep is
        // a tube of radius 0.15 around it, so every vertex sits between those
        // two shells whichever way its ring is turned. A frame *flip* — the
        // failure this whole divergence is here to avoid — would put vertices
        // outside them.
        for (i, point) in mesh.points().iter().enumerate() {
            let radial = (point.x * point.x + point.y * point.y).sqrt();
            assert!(
                (radial - 1.0).abs() <= 0.15 + bound + BBOX_TOLERANCE,
                "{key}: vertex {i} is {radial} from the helix axis, off the tube"
            );
        }
    }

    assert!(checked > 0, "no frame-divergence case was actually checked");
}

/// **Upstream's solid `Extrusion` has an inverted base**, and the port does not.
///
/// `Discretizer::process(Extrusion*)` fans both end caps in the same vertex
/// order — `range<Index>(nbPoints, 0, 1)` at the near end and the same from the
/// last ring — and a cross-section wound counter-clockwise in the frame's
/// `(left, up)` plane fans to a normal along `+heading`. At the far end that
/// points out of the solid; at the near end it points straight in. So every
/// solid `Extrusion` upstream produces has a base that faces the wrong way.
///
/// Nothing upstream notices. `VolComputer` sums the *absolute* tetrahedra about
/// the centroid, so a flipped face changes neither its volume nor the face
/// count nor the area — which is why every other comparison in this file passes
/// on this case regardless. A renderer notices, and so does any signed volume.
///
/// The reference records, per case, how many faces point back towards the
/// middle of the mesh. This asserts three things from it: upstream's count is
/// exactly one cap's fan on the solid sweep, it is zero on every other solid
/// case (so the measure means something and upstream is not being accused
/// wholesale), and the port's own count is zero on the same mesh.
#[test]
fn the_inverted_extrusion_base_is_upstreams_alone() {
    let mut checked = 0;

    for (key, reference) in references() {
        let (case, slices) = split_key(&key);
        // The inward/outward test only means anything on a closed mesh.
        if !reference.solid {
            continue;
        }
        let mesh = discretize_with(
            &build(case, slices).expect("covered"),
            DiscretizeCtx {
                slices,
                stacks: slices,
                curve_samples: 64,
            },
        )
        .expect("discretisation");

        if case == "extrusion_straight_solid" {
            checked += 1;
            // One triangle fan over the cross-section's `slices` points is
            // `slices - 2` triangles, and upstream has exactly that many facing
            // the wrong way: one whole cap, not a stray face.
            assert_eq!(
                reference.inward_faces,
                slices as usize - 2,
                "{key}: upstream's inverted base should be exactly one cap's fan \
                 — if it is now 0 the defect is fixed and the port's reversal \
                 should be removed"
            );
        } else {
            assert_eq!(
                reference.inward_faces, 0,
                "{key}: upstream winds every face of a solid outward except the \
                 extrusion base, so this is either a new upstream defect or a \
                 broken measure"
            );
        }

        assert_eq!(
            inward_faces(&mesh),
            0,
            "{key}: the port must wind every face of a solid outward"
        );
    }

    assert!(checked > 0, "the solid extrusion case was not checked");
}

/// Faces whose normal points back towards the middle of the mesh — the same
/// measure `upstream_measure.py` records, so the two counts are comparable.
fn inward_faces(mesh: &Explicit) -> usize {
    let points = mesh.points();
    let centroid = points
        .iter()
        .fold(Vec3::zeros(), |sum, p| sum + p.coords)
        / points.len() as Real;

    let faces: Vec<Vec<u32>> = match mesh {
        Explicit::TriangleSet(m) => m.indices.iter().map(|f| f.to_vec()).collect(),
        Explicit::QuadSet(m) => m.indices.iter().map(|f| f.to_vec()).collect(),
        Explicit::FaceSet(m) => m.indices.clone(),
        _ => return 0,
    };

    faces
        .iter()
        .filter(|face| face.len() >= 3)
        .filter(|face| {
            let corner = |i: usize| points[face[i] as usize].coords;
            let normal = (corner(1) - corner(0)).cross(&(corner(2) - corner(0)));
            if normal.norm() < 1e-9 {
                return false; // degenerate: counted separately, and not wound either way
            }
            let centre = face
                .iter()
                .fold(Vec3::zeros(), |sum, i| sum + points[*i as usize].coords)
                / face.len() as Real;
            normal.dot(&(centre - centroid)) < 0.0
        })
        .count()
}

/// The other side of that coin: where the port follows upstream's own code
/// path, it reproduces upstream's degenerate faces rather than quietly
/// cleaning them up.
#[test]
fn a_zero_taper_frustum_keeps_upstreams_degenerate_cap() {
    let references = references();
    for slices in [8u8, 32] {
        let reference = &references[&format!("frustum_cone@{slices}")];
        assert_eq!(
            reference.degenerate_faces, slices as usize,
            "upstream's zero-taper frustum should have one degenerate cap face per slice"
        );

        let mesh = discretize_with(
            &build("frustum_cone", slices).unwrap(),
            DiscretizeCtx {
                slices,
                stacks: slices,
                curve_samples: 64,
            },
        )
        .unwrap();
        assert_eq!(
            mesh.face_count(),
            reference.faces,
            "the frustum path is translated verbatim, so its face count must match exactly"
        );
    }
}
