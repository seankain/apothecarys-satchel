use crate::genetics::PlantGenotype;
use crate::lod::LodTier;
use serde::{Deserialize, Serialize};

/// RGBA color representation for plant parts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PlantColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl PlantColor {
    pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> Self {
        let h = hue % 360.0;
        let s = saturation.clamp(0.0, 1.0);
        let v = value.clamp(0.0, 1.0);

        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self {
            r: r + m,
            g: g + m,
            b: b + m,
            a: 1.0,
        }
    }
}

/// How sharply a stem narrows from one internode to the next.
///
/// The turtle can follow an arbitrary radius profile along an axis
/// (`Turtle::n_forward_tapered` with a `QuantisedFunction`), but an L-system
/// axis is not known to be any particular length when its first segment is
/// drawn — the apex may yet be rewritten. So the taper is expressed as the
/// ratio between consecutive segments instead, which is well defined however
/// long the axis turns out to be and compounds to the same exponential
/// narrowing a real stem has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaperCurve {
    /// No narrowing at all — a reed.
    Cylindrical,
    /// A gentle narrowing, the default for a herb.
    #[default]
    Gentle,
    /// A woody stem, visibly thinner at every order of branching.
    Strong,
    /// A whip: the tip is a fraction of the base.
    Whiplike,
}

impl TaperCurve {
    /// How many curves there are, so a gene can be mapped onto them.
    pub const COUNT: usize = 4;

    /// By index, clamped. The index is the "taper curve id" a phenotype
    /// carries and a save file stores.
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => TaperCurve::Cylindrical,
            1 => TaperCurve::Gentle,
            2 => TaperCurve::Strong,
            _ => TaperCurve::Whiplike,
        }
    }

    /// The index [`TaperCurve::from_index`] round-trips.
    pub fn index(self) -> usize {
        match self {
            TaperCurve::Cylindrical => 0,
            TaperCurve::Gentle => 1,
            TaperCurve::Strong => 2,
            TaperCurve::Whiplike => 3,
        }
    }

    /// The width of the next segment as a fraction of this one's.
    pub fn segment_ratio(self) -> f32 {
        match self {
            TaperCurve::Cylindrical => 1.0,
            TaperCurve::Gentle => 0.94,
            TaperCurve::Strong => 0.86,
            TaperCurve::Whiplike => 0.76,
        }
    }
}

/// All visual parameters derived from a genotype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlantPhenotype {
    // L-system parameters
    pub axiom_complexity: u32,
    pub branch_angle: f32,
    pub branch_length: f32,
    pub branch_thickness: f32,
    pub branching_factor: u32,

    // Stem parameters — the shape of the swept axis itself
    /// Which profile the stem is swept from; see
    /// [`surfaces::cross_section`](crate::surfaces::cross_section). Index 0 is
    /// the round default.
    pub cross_section_index: usize,
    /// How fast the stem narrows from segment to segment.
    pub taper_curve: TaperCurve,
    /// How strongly each forward move bends towards gravity — the turtle's
    /// `setElasticity`. Zero switches tropism off.
    pub tropism_elasticity: f32,
    /// Degrees of pitch applied after each drawn segment, which is what makes
    /// an axis arc rather than run dead straight.
    pub axis_curvature: f32,

    // Leaf parameters
    pub leaf_mesh_index: usize,
    pub leaf_scale: f32,
    pub leaf_color: PlantColor,
    pub leaves_per_segment: u32,

    // Flower parameters
    pub produces_flowers: bool,
    pub petal_count: u32,
    pub petal_mesh_index: usize,
    pub petal_color: PlantColor,
    pub petal_scale: f32,

    // Fruit parameters
    pub produces_fruit: bool,
    pub fruit_mesh_index: usize,
    pub fruit_color: PlantColor,
    pub fruit_scale: f32,

    /// The quality tier this plant is to be built at. Not genetic — the
    /// caller sets it; [`express_phenotype`] leaves it at
    /// [`LodTier::Hub`](crate::lod::LodTier::Hub).
    pub lod_tier: LodTier,
}

impl PlantPhenotype {
    /// The same phenotype at another quality tier.
    pub fn with_lod(mut self, tier: LodTier) -> Self {
        self.lod_tier = tier;
        self
    }

    /// How many derivation steps this phenotype takes at its tier — its
    /// `axiom_complexity`, capped by [`LodTier::max_iterations`].
    pub fn iterations(&self) -> u32 {
        self.axiom_complexity.min(self.lod_tier.max_iterations())
    }
}

/// Map a value from [0, 1] to an integer range [min, max].
fn map_range_u32(value: f32, min: u32, max: u32) -> u32 {
    let v = value.clamp(0.0, 1.0);
    let range = max - min;
    min + (v * range as f32).round() as u32
}

/// Map a value from [0, 1] to a float range [min, max].
fn map_range_f32(value: f32, min: f32, max: f32) -> f32 {
    let v = value.clamp(0.0, 1.0);
    min + v * (max - min)
}

/// Map petal count gene expression to discrete petal counts: 3, 4, 5, 6, 8.
fn map_petal_count(value: f32) -> u32 {
    let options = [3, 4, 5, 6, 8];
    let index = ((value.clamp(0.0, 1.0) * (options.len() - 1) as f32).round()) as usize;
    options[index.min(options.len() - 1)]
}

/// Map leaf shape gene to a mesh index (0..4 for different leaf templates).
fn map_leaf_shape(value: f32) -> usize {
    (value.clamp(0.0, 1.0) * 4.0).round() as usize
}

/// Map fruit shape gene to a mesh index (0..3 for different fruit templates).
fn map_fruit_shape(value: f32) -> usize {
    (value.clamp(0.0, 1.0) * 3.0).round() as usize
}

/// Map a value in [0, 1] onto `0..count`.
fn map_index(value: f32, count: usize) -> usize {
    debug_assert!(count > 0);
    (value.clamp(0.0, 1.0) * (count - 1) as f32).round() as usize
}

/// Express a genotype as visual phenotype parameters. This is a pure, deterministic function.
///
/// # Where the stem traits come from
///
/// Phase E (#21) added four stem traits that no gene was written for, because
/// the earlier generator had no way to render them. Rather than widen
/// [`PlantGenotype`] — which would invalidate every save and every stored
/// breeding pair — each rides the gene nearest to it in meaning:
///
/// - **cross-section profile** and **tropism elasticity** ride
///   `stem_thickness`. Both are stem traits, and the second is the honest
///   relationship: a thin stem is a floppy one, so it bends further towards
///   gravity per unit of travel.
/// - **taper curve** rides `internode_length`. A plant that puts long gaps
///   between its nodes narrows faster over the same number of them.
/// - **axis curvature** rides `stem_height`, the one morphology gene the
///   earlier phenotype never expressed. A tall axis arcs under its own length.
pub fn express_phenotype(genotype: &PlantGenotype) -> PlantPhenotype {
    let leaf_hue = map_range_f32(genotype.leaf_color_hue.express(), 60.0, 150.0); // green range
    let leaf_sat = map_range_f32(genotype.leaf_color_saturation.express(), 0.3, 1.0);

    let petal_hue = map_range_f32(genotype.petal_color_hue.express(), 0.0, 360.0);
    let fruit_hue = map_range_f32(genotype.fruit_color_hue.express(), 0.0, 360.0);

    let thickness = genotype.stem_thickness.express();

    PlantPhenotype {
        axiom_complexity: map_range_u32(genotype.branching_density.express(), 1, 6),
        branch_angle: map_range_f32(genotype.branching_angle.express(), 15.0, 60.0),
        branch_length: map_range_f32(genotype.internode_length.express(), 0.1, 2.0),
        branch_thickness: map_range_f32(thickness, 0.01, 0.1),
        branching_factor: map_range_u32(genotype.branching_density.express(), 1, 4),

        cross_section_index: map_index(thickness, crate::surfaces::CROSS_SECTIONS),
        taper_curve: TaperCurve::from_index(map_index(
            genotype.internode_length.express(),
            TaperCurve::COUNT,
        )),
        tropism_elasticity: map_range_f32(1.0 - thickness, 0.0, 0.22),
        axis_curvature: map_range_f32(genotype.stem_height.express(), 0.0, 6.0),

        leaf_mesh_index: map_leaf_shape(genotype.leaf_shape.express()),
        leaf_scale: map_range_f32(genotype.leaf_size.express(), 0.2, 1.5),
        leaf_color: PlantColor::from_hsv(leaf_hue, leaf_sat, 0.7),
        leaves_per_segment: map_range_u32(genotype.leaf_density.express(), 1, 5),

        produces_flowers: genotype.has_flowers.express() > 0.5,
        petal_count: map_petal_count(genotype.petal_count.express()),
        petal_mesh_index: map_index(
            genotype.flower_density.express(),
            crate::surfaces::PETAL_SHAPES,
        ),
        petal_color: PlantColor::from_hsv(petal_hue, 0.8, 0.9),
        petal_scale: map_range_f32(genotype.petal_size.express(), 0.1, 0.5),

        produces_fruit: genotype.has_fruit.express() > 0.5,
        fruit_mesh_index: map_fruit_shape(genotype.fruit_shape.express()),
        fruit_color: PlantColor::from_hsv(fruit_hue, 0.7, 0.8),
        fruit_scale: map_range_f32(genotype.fruit_size.express(), 0.1, 0.6),

        lod_tier: LodTier::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genetics::PlantGenotype;
    use rand::SeedableRng;

    #[test]
    fn test_express_phenotype_deterministic() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let genotype = PlantGenotype::random_wild(&mut rng);

        let p1 = express_phenotype(&genotype);
        let p2 = express_phenotype(&genotype);
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_phenotype_values_in_range() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let genotype = PlantGenotype::random_wild(&mut rng);
            let p = express_phenotype(&genotype);

            assert!((1..=6).contains(&p.axiom_complexity));
            assert!((15.0..=60.0).contains(&p.branch_angle));
            assert!((0.1..=2.0).contains(&p.branch_length));
            assert!((0.01..=0.1).contains(&p.branch_thickness));
            assert!((1..=4).contains(&p.branching_factor));
            assert!(p.leaf_mesh_index <= 4);
            assert!((0.2..=1.5).contains(&p.leaf_scale));
            assert!((1..=5).contains(&p.leaves_per_segment));
            assert!([3, 4, 5, 6, 8].contains(&p.petal_count));
            assert!((0.1..=0.5).contains(&p.petal_scale));
            assert!(p.fruit_mesh_index <= 3);
            assert!((0.1..=0.6).contains(&p.fruit_scale));

            assert!(p.cross_section_index < crate::surfaces::CROSS_SECTIONS);
            assert!(p.petal_mesh_index < crate::surfaces::PETAL_SHAPES);
            assert!((0.0..=0.22).contains(&p.tropism_elasticity));
            assert!((0.0..=6.0).contains(&p.axis_curvature));
            assert_eq!(p.lod_tier, LodTier::Hub);
        }
    }

    #[test]
    fn test_taper_curve_index_round_trips() {
        for index in 0..TaperCurve::COUNT {
            assert_eq!(TaperCurve::from_index(index).index(), index);
        }
        // Past the end clamps to the last curve rather than panicking.
        assert_eq!(TaperCurve::from_index(99), TaperCurve::Whiplike);
    }

    #[test]
    fn test_taper_ratios_are_ordered_and_bounded() {
        let mut previous = f32::INFINITY;
        for index in 0..TaperCurve::COUNT {
            let ratio = TaperCurve::from_index(index).segment_ratio();
            assert!((0.0..=1.0).contains(&ratio));
            assert!(ratio < previous, "curve {index} does not taper harder");
            previous = ratio;
        }
    }

    #[test]
    fn test_lod_tier_caps_the_derivation() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let mut phenotype = express_phenotype(&genotype);
        phenotype.axiom_complexity = 6;

        assert_eq!(phenotype.clone().with_lod(LodTier::Hub).iterations(), 6);
        assert_eq!(phenotype.clone().with_lod(LodTier::Distant).iterations(), 5);
        assert_eq!(phenotype.clone().with_lod(LodTier::Icon).iterations(), 3);

        // A phenotype that asks for less than the tier allows keeps its own.
        phenotype.axiom_complexity = 2;
        assert_eq!(phenotype.with_lod(LodTier::Hub).iterations(), 2);
    }

    #[test]
    fn test_different_genotypes_produce_different_phenotypes() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let g1 = PlantGenotype::random_wild(&mut rng);
        let g2 = PlantGenotype::random_wild(&mut rng);

        let p1 = express_phenotype(&g1);
        let p2 = express_phenotype(&g2);

        // They should differ in at least some parameters
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_map_petal_count_discrete() {
        assert_eq!(map_petal_count(0.0), 3);
        assert_eq!(map_petal_count(0.25), 4);
        assert_eq!(map_petal_count(0.5), 5);
        assert_eq!(map_petal_count(0.75), 6);
        assert_eq!(map_petal_count(1.0), 8);
    }

    #[test]
    fn test_hsv_to_rgb() {
        // Red: HSV(0, 1, 1) → RGB(1, 0, 0)
        let red = PlantColor::from_hsv(0.0, 1.0, 1.0);
        assert!((red.r - 1.0).abs() < 0.01);
        assert!(red.g < 0.01);
        assert!(red.b < 0.01);

        // Green: HSV(120, 1, 1) → RGB(0, 1, 0)
        let green = PlantColor::from_hsv(120.0, 1.0, 1.0);
        assert!(green.r < 0.01);
        assert!((green.g - 1.0).abs() < 0.01);
        assert!(green.b < 0.01);
    }

    #[test]
    fn test_phenotype_serde_roundtrip() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype);

        let json = serde_json::to_string(&phenotype).unwrap();
        let deserialized: PlantPhenotype = serde_json::from_str(&json).unwrap();
        assert_eq!(phenotype, deserialized);
    }
}
