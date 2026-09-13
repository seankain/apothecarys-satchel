//! Snapshots of the plant generator as it stands *before* the PlantGL port.
//!
//! T8.3 (#17) asks for these to exist before any Phase E migration, so the
//! change in geometry that `crates/plantgl` brings is a reviewable diff rather
//! than something noticed after the fact. They deliberately snapshot the
//! current hand-rolled `PlantMeshData::to_obj`/`to_mtl` output — not anything
//! routed through `plantgl` — because that output is the baseline.
//!
//! When Phase E rewires `crates/botany` onto `plantgl`, do not quietly rewrite
//! these files: read the diff, and keep a copy of the old ones alongside if
//! the comparison is worth preserving.
//!
//! `UPDATE_GOLDEN=1 cargo test -p apothecarys-botany --test golden_pre_plantgl`
//! rewrites them.

use std::path::PathBuf;

use apothecarys_botany::genetics::PlantGenotype;
use apothecarys_botany::mesh_gen::generate_plant_mesh;
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
        "\n{name} differs from its golden. This is the pre-port baseline: \
         read the diff before rewriting it."
    );
}

/// Reproduces `PlantPreviewData::from_seed`'s pipeline without pulling in the
/// tools crate and its optional engine dependency.
fn generate(seed: u64) -> apothecarys_botany::mesh_gen::PlantMeshData {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let genotype = PlantGenotype::random_wild(&mut rng);
    generate_plant_mesh(&genotype, &mut rng)
}

#[test]
fn current_generator_obj_snapshots() {
    for seed in SEEDS {
        let mesh = generate(seed);
        assert_golden(
            &format!("plant_seed_{seed}.obj"),
            &mesh.to_obj(&format!("plant_seed_{seed}.mtl")),
        );
        assert_golden(&format!("plant_seed_{seed}.mtl"), &mesh.to_mtl());
    }
}

/// The snapshots are only meaningful if the generator is deterministic for a
/// seed, which saves already rely on.
#[test]
fn current_generator_is_deterministic() {
    for seed in SEEDS {
        let first = generate(seed);
        let second = generate(seed);
        assert_eq!(
            first.to_obj("x.mtl"),
            second.to_obj("x.mtl"),
            "seed {seed} is not deterministic"
        );
    }
}
