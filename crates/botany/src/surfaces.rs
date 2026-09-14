//! The organ templates the turtle instances and the profiles it sweeps.
//!
//! MIT, like the rest of `crates/botany`. Nothing here is translated from
//! PlantGL: it *builds* `plantgl` types — [`BezierPatch`] blades,
//! [`Polyline2D`] cross-sections, [`Sphere`] fruit bodies — out of phenotype
//! indices, which is the Derivative Software side of the CeCILL-C boundary
//! (#17). Keep it that way.
//!
//! # Local frame
//!
//! A surface is modelled in the turtle's *local* frame and placed by
//! `plantgl::modelling::geometry::place`, whose basis sends local `+X` to the
//! turtle's `up`, local `+Y` to `-left` and local `+Z` to `heading`. So a
//! blade runs from `z = 0` at its attachment to `z = 1` at its tip, spreads
//! along `±y`, and cups or droops along `x`. That is the same convention
//! upstream's default `"l"` leaf uses, so a surface written here drops into a
//! cpfg program unchanged.

use plantgl::math::{Point2, Point3, Real, Vec3};
use plantgl::modelling::SurfaceLibrary;
use plantgl::scenegraph::{
    BezierPatch, CtrlPointMatrix, Curve2D, Curve2DRef, Geometry, GeometryRef, Polyline2D, Sphere,
    Transform, Transformed,
};
use serde::{Deserialize, Serialize};

use crate::lod::LodTier;

/// How many shapes each organ kind offers. The leaf and fruit counts are the
/// ranges `phenotype::map_leaf_shape` and `map_fruit_shape` already produce,
/// so the existing `leaf_mesh_index` / `fruit_mesh_index` genes index straight
/// into them.
pub const LEAF_SHAPES: usize = 5;
pub const PETAL_SHAPES: usize = 3;
pub const FRUIT_SHAPES: usize = 4;

/// Which kind of organ a surface is — what decides its material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Organ {
    Leaf,
    Petal,
    Fruit,
}

/// A template in the plant's [`SurfaceLibrary`], named by kind and index.
///
/// This is what [`LSymbol::Surface`](crate::lsystem::LSymbol::Surface) carries:
/// the L-system says *which* organ goes here, the library says what it looks
/// like, and the turtle's frame says where it points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SurfaceId {
    /// A leaf blade, `0..LEAF_SHAPES`.
    Leaf(usize),
    /// A single petal, `0..PETAL_SHAPES`. A flower is several of these rolled
    /// around the axis.
    Petal(usize),
    /// A fruit body, `0..FRUIT_SHAPES`.
    Fruit(usize),
}

impl SurfaceId {
    /// The organ kind, which picks the material.
    pub fn organ(self) -> Organ {
        match self {
            SurfaceId::Leaf(_) => Organ::Leaf,
            SurfaceId::Petal(_) => Organ::Petal,
            SurfaceId::Fruit(_) => Organ::Fruit,
        }
    }

    /// The shape index, clamped to the number of shapes that kind has.
    pub fn index(self) -> usize {
        match self {
            SurfaceId::Leaf(i) => i.min(LEAF_SHAPES - 1),
            SurfaceId::Petal(i) => i.min(PETAL_SHAPES - 1),
            SurfaceId::Fruit(i) => i.min(FRUIT_SHAPES - 1),
        }
    }

    /// The key this template is registered under — what
    /// `Turtle::surface(name, scale)` is given.
    ///
    /// Out-of-range indices clamp rather than fail: a gene that drifts past
    /// the end of the shape table should give the last leaf, not no leaf.
    pub fn name(self) -> &'static str {
        const LEAVES: [&str; LEAF_SHAPES] = ["leaf0", "leaf1", "leaf2", "leaf3", "leaf4"];
        const PETALS: [&str; PETAL_SHAPES] = ["petal0", "petal1", "petal2"];
        const FRUITS: [&str; FRUIT_SHAPES] = ["fruit0", "fruit1", "fruit2", "fruit3"];
        match self {
            SurfaceId::Leaf(_) => LEAVES[self.index()],
            SurfaceId::Petal(_) => PETALS[self.index()],
            SurfaceId::Fruit(_) => FRUITS[self.index()],
        }
    }
}

/// Every organ template, built at the density `lod` asks for.
///
/// One library serves a whole plant — and, because the geometries are
/// `Arc`ed, a whole garden at the same tier if the caller keeps it.
pub fn organ_library(lod: LodTier) -> SurfaceLibrary {
    let mut library = SurfaceLibrary::new();
    let (u, v) = lod.patch_strides();
    for index in 0..LEAF_SHAPES {
        library.insert(SurfaceId::Leaf(index).name(), leaf_blade(index, u, v));
    }
    for index in 0..PETAL_SHAPES {
        library.insert(SurfaceId::Petal(index).name(), petal_blade(index, u, v));
    }
    let (slices, stacks) = lod.fruit_resolution();
    for index in 0..FRUIT_SHAPES {
        library.insert(SurfaceId::Fruit(index).name(), fruit_body(index, slices, stacks));
    }
    library
}

/// The half-widths of a blade at its four control rows, from attachment to
/// tip. The tip is always zero, which is what makes a blade pointed rather
/// than cropped.
type Profile = [Real; 4];

/// The five leaf outlines, in the order `leaf_mesh_index` produces them.
const LEAF_PROFILES: [(Profile, Real, Real); LEAF_SHAPES] = [
    // (half-widths, cup across the blade, droop along it)
    ([0.04, 0.16, 0.12, 0.0], 0.30, 0.10), // 0 lanceolate — narrow, tapering
    ([0.06, 0.26, 0.14, 0.0], 0.35, 0.14), // 1 ovate — widest low down
    ([0.03, 0.20, 0.20, 0.0], 0.25, 0.08), // 2 elliptic — widest in the middle
    ([0.02, 0.36, 0.18, 0.0], 0.45, 0.18), // 3 cordate — broad, heart-like base
    ([0.03, 0.12, 0.28, 0.0], 0.30, 0.16), // 4 obovate — widest near the tip
];

/// The three petal outlines. Petals are shorter, broader at the base and cup
/// harder than leaves, which is what reads as a flower rather than a rosette.
const PETAL_PROFILES: [(Profile, Real, Real); PETAL_SHAPES] = [
    ([0.10, 0.24, 0.22, 0.0], 0.55, 0.06), // 0 rounded
    ([0.05, 0.18, 0.10, 0.0], 0.40, 0.04), // 1 pointed
    ([0.14, 0.32, 0.26, 0.0], 0.65, 0.10), // 2 broad
];

/// The four fruit bodies, as per-axis scales of a unit-diameter sphere. `z`
/// runs along the turtle's heading, so a stretched `z` hangs a pod off the
/// stem's end rather than widening a berry.
const FRUIT_SCALES: [[Real; 3]; FRUIT_SHAPES] = [
    [1.00, 1.00, 1.00], // 0 berry
    [0.85, 0.85, 1.35], // 1 drupe
    [0.55, 0.55, 1.90], // 2 pod
    [1.30, 1.30, 0.70], // 3 capsule
];

/// A blade as a 4×4 Bézier patch: rows run from attachment to tip, columns
/// across the blade.
///
/// The two interior columns sit at a third of the half-width and are lifted
/// along `x` by `cup`, so the cross-section arches instead of being a flat
/// ribbon; the rows sag by `droop · z²`, so the blade bends over under its own
/// length. Both are control-point offsets, not sampled displacements — the
/// patch stays a patch, and the LOD strides decide how finely it is read.
fn blade(profile: Profile, cup: Real, droop: Real, u_stride: u32, v_stride: u32) -> GeometryRef {
    let rows: Vec<Vec<Point3>> = (0..4)
        .map(|i| {
            let z = i as Real / 3.0;
            let half = profile[i];
            let sag = -droop * z * z;
            let lift = sag + cup * half;
            vec![
                Point3::new(sag, -half, z),
                Point3::new(lift, -half / 3.0, z),
                Point3::new(lift, half / 3.0, z),
                Point3::new(sag, half, z),
            ]
        })
        .collect();
    // `from_point_rows` only fails on a ragged or too-small net, and this one
    // is a literal 4×4.
    let net = CtrlPointMatrix::from_point_rows(rows).expect("a 4x4 control net is well formed");
    Geometry::from(BezierPatch::new(net).with_strides(u_stride.max(2), v_stride.max(2))).into_ref()
}

fn leaf_blade(index: usize, u_stride: u32, v_stride: u32) -> GeometryRef {
    let (profile, cup, droop) = LEAF_PROFILES[index.min(LEAF_SHAPES - 1)];
    blade(profile, cup, droop, u_stride, v_stride)
}

fn petal_blade(index: usize, u_stride: u32, v_stride: u32) -> GeometryRef {
    let (profile, cup, droop) = PETAL_PROFILES[index.min(PETAL_SHAPES - 1)];
    blade(profile, cup, droop, u_stride, v_stride)
}

/// A fruit: a unit sphere scaled along the three local axes and pushed half a
/// diameter up the heading, so it hangs off the end of the stem it was drawn
/// at rather than swallowing it.
fn fruit_body(index: usize, slices: u8, stacks: u8) -> GeometryRef {
    let scale = FRUIT_SCALES[index.min(FRUIT_SHAPES - 1)];
    let body = Geometry::from(Sphere::new(0.5, slices.max(3), stacks.max(2))).into_ref();
    let scaled =
        Geometry::from(Transformed::new(Transform::Scaled(Vec3::new(scale[0], scale[1], scale[2])), body));
    Geometry::from(Transformed::new(
        Transform::Translated(Vec3::new(0.0, 0.0, scale[2] * 0.5)),
        scaled.into_ref(),
    ))
    .into_ref()
}

/// How many stem profiles [`cross_section`] offers.
pub const CROSS_SECTIONS: usize = 4;

/// The profile a stem is swept from, by index.
///
/// `None` is the round default, which the turtle already installs for itself
/// at its own section resolution — asking for it explicitly would only pin the
/// facet count and defeat the LOD tier. The other three are the shapes a
/// cylinder cannot make: the square stem of a mint, the triangular one of a
/// sedge, the fluted one of a grass.
pub fn cross_section(index: usize, resolution: u32) -> Option<Curve2DRef> {
    let closed = |points: Vec<Point2>| {
        let mut points = points;
        // A swept profile closes without a seam only if its ends coincide
        // exactly, as `Polyline2D::circle` is careful to arrange.
        if let Some(first) = points.first().copied() {
            points.push(first);
        }
        Some(Curve2D::from(Polyline2D::new(points)).into_ref())
    };
    match index {
        0 => None,
        1 => closed(regular_polygon(4, std::f32::consts::FRAC_PI_4)),
        2 => closed(regular_polygon(3, std::f32::consts::FRAC_PI_2)),
        _ => closed(fluted(resolution.clamp(6, 24) as usize)),
    }
}

fn regular_polygon(sides: usize, phase: Real) -> Vec<Point2> {
    (0..sides)
        .map(|i| {
            let angle = phase + std::f32::consts::TAU * i as Real / sides as Real;
            Point2::new(angle.cos(), angle.sin())
        })
        .collect()
}

/// A ridged profile: alternating radii around the circle, so a sweep of it has
/// visible flutes running up the stem.
fn fluted(points: usize) -> Vec<Point2> {
    let points = points + points % 2; // an even count, so the ridges alternate
    (0..points)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as Real / points as Real;
            let radius = if i % 2 == 0 { 1.0 } else { 0.72 };
            Point2::new(radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

/// The names every template in a library built by [`organ_library`] is
/// registered under, for tests and for tooling that wants to list them.
pub fn surface_names() -> Vec<&'static str> {
    let mut names = Vec::with_capacity(LEAF_SHAPES + PETAL_SHAPES + FRUIT_SHAPES);
    names.extend((0..LEAF_SHAPES).map(|i| SurfaceId::Leaf(i).name()));
    names.extend((0..PETAL_SHAPES).map(|i| SurfaceId::Petal(i).name()));
    names.extend((0..FRUIT_SHAPES).map(|i| SurfaceId::Fruit(i).name()));
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use plantgl::algo::{discretize::discretize, measure::surface_area};

    #[test]
    fn the_library_holds_every_template() {
        let library = organ_library(LodTier::Hub);
        assert_eq!(library.len(), LEAF_SHAPES + PETAL_SHAPES + FRUIT_SHAPES);
        for name in surface_names() {
            assert!(library.get(name).is_some(), "{name} is missing");
        }
    }

    #[test]
    fn every_blade_has_area_and_sits_in_its_local_frame() {
        let library = organ_library(LodTier::Hub);
        for index in 0..LEAF_SHAPES {
            let leaf = library.get(SurfaceId::Leaf(index).name()).unwrap();
            let mesh = discretize(leaf).unwrap();
            let area = surface_area(&mesh).unwrap();
            assert!(area > 0.0, "leaf {index} has no area");
            // A blade runs from its attachment to a tip one unit along the
            // heading, and never crosses back behind the attachment.
            let bbox = plantgl::algo::bounding_box(leaf).unwrap().unwrap();
            assert!(bbox.lower_left.z >= -1e-5, "leaf {index}: {bbox:?}");
            assert!((bbox.upper_right.z - 1.0).abs() < 1e-5, "leaf {index}: {bbox:?}");
        }
    }

    #[test]
    fn a_coarser_tier_gives_a_smaller_blade_mesh() {
        let count = |tier: LodTier| {
            let library = organ_library(tier);
            let leaf = library.get(SurfaceId::Leaf(1).name()).unwrap();
            let mesh = discretize(leaf).unwrap();
            plantgl::algo::tessellate(&mesh).unwrap().face_count()
        };
        assert!(count(LodTier::Hub) > count(LodTier::Distant));
        assert!(count(LodTier::Distant) >= count(LodTier::Icon));
    }

    #[test]
    fn a_fruit_hangs_off_the_end_rather_than_around_it() {
        let library = organ_library(LodTier::Hub);
        for index in 0..FRUIT_SHAPES {
            let fruit = library.get(SurfaceId::Fruit(index).name()).unwrap();
            let bbox = plantgl::algo::bounding_box(fruit).unwrap().unwrap();
            assert!(bbox.lower_left.z >= -1e-4, "fruit {index}: {bbox:?}");
            assert!(bbox.upper_right.z > 0.5, "fruit {index}: {bbox:?}");
        }
    }

    #[test]
    fn an_out_of_range_index_clamps_instead_of_panicking() {
        assert_eq!(SurfaceId::Leaf(99).name(), SurfaceId::Leaf(LEAF_SHAPES - 1).name());
        assert_eq!(SurfaceId::Petal(99).name(), SurfaceId::Petal(PETAL_SHAPES - 1).name());
        assert_eq!(SurfaceId::Fruit(99).name(), SurfaceId::Fruit(FRUIT_SHAPES - 1).name());
    }

    #[test]
    fn the_round_profile_defers_to_the_turtles_own() {
        assert!(cross_section(0, 8).is_none());
        for index in 1..CROSS_SECTIONS {
            let profile = cross_section(index, 8).expect("a profile");
            profile.is_valid().expect("a valid profile");
        }
    }

    #[test]
    fn a_swept_profile_closes_on_itself() {
        for index in 1..CROSS_SECTIONS {
            let profile = cross_section(index, 8).unwrap();
            let Curve2D::Polyline2D(line) = profile.as_ref() else {
                panic!("profile {index} is not a polyline");
            };
            let first = line.points.first().copied().unwrap();
            let last = line.points.last().copied().unwrap();
            assert!((first - last).norm() < 1e-6, "profile {index} has a seam");
        }
    }
}
