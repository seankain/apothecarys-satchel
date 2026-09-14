//! OBJ/MTL snapshots of the plant generator.
//!
//! # The pre-port baseline
//!
//! `tests/golden/pre-plantgl/` holds the snapshots T8.3 (#17) asked for before
//! any of the migration landed: the hand-rolled `PlantMeshData::to_obj` output,
//! two-ring cylinders per internode and a marker triangle per organ. They are
//! kept, not asserted — the generator that produced them no longer exists —
//! so the change Phase E made to the geometry stays reviewable. Do not update
//! them; they are a historical record.
//!
//! `tests/golden/` holds the current snapshots, written by `plantgl::codec`
//! from the merged, discretised scene. These *are* asserted, on the same five
//! seeds, so a change in the sweeps, the organ patches or the LOD densities
//! shows up as a diff rather than as something noticed in a screenshot weeks
//! later.
//!
//! `UPDATE_GOLDEN=1 cargo test -p apothecarys-botany --test golden` rewrites
//! them. Read the diff before committing it.

use std::path::PathBuf;

use apothecarys_botany::genetics::PlantGenotype;
use apothecarys_botany::interpret::{generate_plant_mesh, PlantModel};
use rand::SeedableRng;

/// The seeds `PlantPreviewData::from_seed` is exercised with in
/// `crates/tools`, so the snapshots cover the shapes a reviewer has seen.
const SEEDS: [u64; 5] = [1, 42, 100, 999, 12345];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn assert_golden(name: &str, content: &str) {
    let path = golden_dir().join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden dir")).expect("create golden dir");
        std::fs::write(&path, content).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}. Re-run with UPDATE_GOLDEN=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        content, expected,
        "\n{name} differs from its golden. Read the diff before rewriting it."
    );
}

/// Reproduces `PlantPreviewData::from_seed`'s pipeline without pulling in the
/// tools crate and its optional engine dependency.
fn generate(seed: u64) -> PlantModel {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let genotype = PlantGenotype::random_wild(&mut rng);
    generate_plant_mesh(&genotype, &mut rng).expect("a plant")
}

#[test]
fn generator_obj_snapshots() {
    for seed in SEEDS {
        let files = generate(seed)
            .to_obj(&format!("plant_seed_{seed}.mtl"))
            .expect("an OBJ export");
        assert_golden(&format!("plant_seed_{seed}.obj"), &files.obj);
        assert_golden(&format!("plant_seed_{seed}.mtl"), &files.mtl);
    }
}

/// The snapshots are only meaningful if the generator is deterministic for a
/// seed, which saves already rely on.
#[test]
fn the_generator_is_deterministic() {
    for seed in SEEDS {
        let first = generate(seed).to_obj("x.mtl").expect("an OBJ export");
        let second = generate(seed).to_obj("x.mtl").expect("an OBJ export");
        assert_eq!(first, second, "seed {seed} is not deterministic");
    }
}

/// The pre-port snapshots stay on disk as the thing to compare against.
#[test]
fn the_pre_port_baseline_is_still_there() {
    for seed in SEEDS {
        for extension in ["obj", "mtl"] {
            let path = golden_dir().join(format!("pre-plantgl/plant_seed_{seed}.{extension}"));
            assert!(
                path.exists(),
                "{} is missing: it is the T8.3 baseline and should not be deleted",
                path.display()
            );
        }
    }
}
