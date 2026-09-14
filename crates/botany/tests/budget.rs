//! The performance budget from #21, enforced.
//!
//! | Operation | Budget |
//! |---|---|
//! | Genotype → derived L-system string | < 1 ms |
//! | Turtle interpretation + GC construction | < 3 ms |
//! | Discretise + tessellate + merge, hub LOD | < 10 ms |
//! | **Total per plant, hub LOD** | **< 15 ms** |
//! | Triangles per plant — hub / distant / icon | < 12 000 / < 1 500 / < 400 |
//!
//! Plants are built at garden load and on harvest, not per frame, and a
//! twelve-plot garden at 15 ms each is ~180 ms serially — or nothing at all
//! across `rayon`, which stays available because `plantgl` shares through
//! `Arc` and there is no FFI boundary.
//!
//! # Reading a failure
//!
//! The triangle assertions are exact and deterministic: a failure is a real
//! regression, and the lever is [`LodTier`] — `section_resolution` first,
//! `patch_strides` second, `max_iterations` last.
//!
//! The timing assertions are only meaningful in a release build, where the
//! numbers above were measured; a debug build runs the meshing roughly forty
//! times slower, so the thresholds are scaled by [`DEBUG_SLACK`] there and the
//! test degrades to a check that nothing has become pathologically slow. The
//! real gate is:
//!
//! ```text
//! cargo test --release -p apothecarys-botany --test budget
//! ```
//!
//! Each stage is timed as the *best* of several runs rather than the mean, so
//! a scheduler hiccup on a loaded machine cannot fail the build.

use std::time::{Duration, Instant};

use apothecarys_botany::genetics::PlantGenotype;
use apothecarys_botany::interpret::{build_scene, generate_plant_at, measure, PlantModel};
use apothecarys_botany::lod::LodTier;
use apothecarys_botany::lsystem::LSystem;
use apothecarys_botany::phenotype::express_phenotype;
use rand::SeedableRng;

/// How much slower a debug build is allowed to be than the release budget.
const DEBUG_SLACK: u32 = 40;

/// How many seeds the triangle budget is checked over. Wide enough to catch a
/// genotype at the far end of every range; cheap enough to stay in the default
/// test run.
const SEEDS: u64 = 120;

/// The seed that drew the most triangles when the budget was last measured —
/// the one worth timing.
const HEAVIEST_SEED: u64 = 285;

fn seeded(seed: u64) -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(seed)
}

fn plant(seed: u64, tier: LodTier) -> PlantModel {
    let mut rng = seeded(seed);
    let genotype = PlantGenotype::random_wild(&mut rng);
    generate_plant_at(&genotype, &mut rng, tier).expect("a plant")
}

fn budget(limit: Duration) -> Duration {
    if cfg!(debug_assertions) {
        limit * DEBUG_SLACK
    } else {
        limit
    }
}

/// The best of `runs` timings of `f`, after one warm-up.
fn best_of(runs: u32, mut f: impl FnMut()) -> Duration {
    f();
    (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .min()
        .expect("at least one run")
}

#[test]
fn every_tier_stays_inside_its_triangle_budget() {
    for tier in LodTier::ALL {
        let mut worst = (0usize, 0u64);
        for seed in 0..SEEDS {
            let triangles = plant(seed, tier).triangle_count().expect("a mesh");
            if triangles > worst.0 {
                worst = (triangles, seed);
            }
        }
        assert!(
            worst.0 <= tier.triangle_budget(),
            "{tier:?}: seed {} drew {} triangles, over the budget of {}",
            worst.1,
            worst.0,
            tier.triangle_budget()
        );
        // A tier that never gets near its budget is a tier drawing worse
        // plants than it was paid for.
        assert!(
            worst.0 * 20 >= tier.triangle_budget(),
            "{tier:?}: the heaviest plant of {} seeds is only {} triangles against a \
             budget of {} — the tier is leaving quality on the table",
            SEEDS,
            worst.0,
            tier.triangle_budget()
        );
    }
}

/// A plant has to reach the renderer as a handful of draw calls, which is what
/// merging by appearance before conversion buys.
#[test]
fn a_plant_is_a_handful_of_draw_calls() {
    for seed in 0..SEEDS {
        let batches = plant(seed, LodTier::Hub).batches().expect("batches");
        assert!(
            batches.len() <= 4,
            "seed {seed} needs {} draw calls; one per organ kind plus the stem is the most \
             there should be",
            batches.len()
        );
    }
}

#[test]
fn the_per_stage_time_budget_holds() {
    let mut rng = seeded(HEAVIEST_SEED);
    let genotype = PlantGenotype::random_wild(&mut rng);
    let phenotype = express_phenotype(&genotype);
    let lsystem = LSystem::from_phenotype(&phenotype);
    assert_eq!(
        phenotype.iterations(),
        LodTier::Hub.max_iterations(),
        "the timing seed should exercise a full-depth derivation"
    );

    let derive = best_of(20, || {
        let mut rng = seeded(HEAVIEST_SEED);
        let _ = PlantGenotype::random_wild(&mut rng);
        std::hint::black_box(lsystem.derive(phenotype.iterations(), &mut rng));
    });

    // The string every later stage works on, derived once.
    let mut rng = seeded(HEAVIEST_SEED);
    let _ = PlantGenotype::random_wild(&mut rng);
    let symbols = lsystem.derive(phenotype.iterations(), &mut rng);

    let interpret = best_of(20, || {
        std::hint::black_box(build_scene(&symbols, &phenotype).expect("a scene"));
    });

    let (scene, stats) = build_scene(&symbols, &phenotype).expect("a scene");
    let model = PlantModel::new(scene, phenotype.clone(), stats, symbols.clone());
    let mesh = best_of(10, || {
        std::hint::black_box(model.batches().expect("batches"));
    });

    let total = derive + interpret + mesh;
    let report = format!(
        "derive {derive:?}, interpret {interpret:?}, mesh {mesh:?}, total {total:?} \
         ({} triangles)",
        model.triangle_count().expect("a mesh")
    );

    assert!(
        derive <= budget(Duration::from_millis(1)),
        "derivation is over budget: {report}"
    );
    assert!(
        interpret <= budget(Duration::from_millis(3)),
        "turtle interpretation is over budget: {report}"
    );
    assert!(
        mesh <= budget(Duration::from_millis(10)),
        "meshing is over budget: {report}"
    );
    assert!(
        total <= budget(Duration::from_millis(15)),
        "the plant is over its total budget: {report}"
    );
}

/// The yield path substitutes for the mesh path rather than adding to it: a
/// harvest reads surface area and volume off the turtle without ever building
/// geometry, so it answers to the same 15 ms a rendered plant does.
#[test]
fn the_measurement_path_stays_inside_the_whole_plant_budget() {
    let mut rng = seeded(HEAVIEST_SEED);
    let genotype = PlantGenotype::random_wild(&mut rng);
    let phenotype = express_phenotype(&genotype);
    let lsystem = LSystem::from_phenotype(&phenotype);
    let symbols = lsystem.derive(phenotype.iterations(), &mut rng);

    let elapsed = best_of(10, || {
        std::hint::black_box(measure(&symbols, &phenotype).expect("measurements"));
    });
    let (measures, _) = measure(&symbols, &phenotype).expect("measurements");
    assert!(measures.surface_area > 0.0 && measures.volume > 0.0);
    assert!(
        elapsed <= budget(Duration::from_millis(15)),
        "measuring a plant took {elapsed:?} ({} segments)",
        measures.segment_count
    );
}

/// Twelve plots is a full garden. Serially it has to fit inside a load screen.
#[test]
fn a_full_garden_builds_in_one_pass() {
    let elapsed = best_of(3, || {
        for seed in 0..12u64 {
            std::hint::black_box(plant(seed, LodTier::Hub).batches().expect("batches"));
        }
    });
    assert!(
        elapsed <= budget(Duration::from_millis(15 * 12)),
        "a twelve-plot garden took {elapsed:?}"
    );
}
