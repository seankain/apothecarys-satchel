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
use plantgl::math::{Point2, Real, Vec3};
use plantgl::scenegraph::curve::{Curve2D, Polyline2D};
use plantgl::scenegraph::primitive::{
    Box3, Cone, Cylinder, Disc, ElevationGrid, Frustum, HeightField, Paraboloid, Revolution,
    Sphere,
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
        _ => return None,
    })
}

// --- Reading the reference --------------------------------------------------

/// One upstream measurement, parsed from the committed JSON.
#[derive(Debug, Clone)]
struct Reference {
    points: usize,
    faces: usize,
    triangles: usize,
    degenerate_faces: usize,
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

    // And the coverage the issue asks for: every T8.4 primitive, at two slice
    // counts each.
    for required in [
        "box", "sphere", "cone", "cylinder", "frustum", "disc", "paraboloid", "revolution",
        "elevation_grid",
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
            if (ours - theirs).abs() > BBOX_TOLERANCE {
                failures.push(format!(
                    "{key}: bbox component {axis} ours={ours} upstream={theirs}"
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

        let (tolerance, kind) = if reference.area_measures_mesh {
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
