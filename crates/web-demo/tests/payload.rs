//! The wasm boundary, exercised natively.
//!
//! `web/main.js` decodes [`mesh_payload`]'s bytes in a browser, where a
//! mistake shows up as an empty canvas and nothing else. These tests decode
//! the same bytes here, where a mistake shows up as a failing `cargo test`,
//! and check the parts the page relies on: that the layout round-trips, that
//! the geometry matches what `PlantModel` says it should be, that a seed is
//! reproducible, and that different seeds really do give different plants.

use apothecarys_botany::lod::LodTier;
use apothecarys_web_demo::{
    batches, decode, grow, lod_from_index, mesh_payload, plant_info, Batch, MAGIC, VERSION,
};

/// The seeds `crates/botany/tests/golden.rs` snapshots, so a reviewer looking
/// at both is looking at the same five plants.
const SEEDS: [u64; 5] = [1, 42, 100, 999, 12345];

fn batches_for(seed: u64, lod: LodTier) -> Vec<Batch> {
    let (plant, _) = grow(seed, lod).expect("a plant");
    batches(&plant).expect("batches")
}

#[test]
fn the_payload_round_trips() {
    for seed in SEEDS {
        let original = batches_for(seed, LodTier::Hub);
        let decoded = decode(&mesh_payload(&original)).expect("a decodable payload");
        assert_eq!(
            original, decoded,
            "seed {seed} did not survive the encoding"
        );
    }
}

#[test]
fn the_prelude_is_what_the_page_checks_for() {
    let bytes = mesh_payload(&batches_for(42, LodTier::Hub));
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), MAGIC);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), VERSION);
    assert_eq!(
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
        batches_for(42, LodTier::Hub).len()
    );
}

#[test]
fn a_wrong_magic_or_version_is_refused_rather_than_drawn() {
    let mut bytes = mesh_payload(&batches_for(42, LodTier::Hub));
    bytes[0] ^= 0xff;
    assert!(decode(&bytes).is_err(), "a corrupt magic decoded anyway");

    let mut bytes = mesh_payload(&batches_for(42, LodTier::Hub));
    bytes[4..8].copy_from_slice(&(VERSION + 1).to_le_bytes());
    assert!(decode(&bytes).is_err(), "a future version decoded anyway");

    assert!(decode(&[]).is_err(), "an empty payload decoded anyway");
}

#[test]
fn every_batch_is_drawable() {
    for seed in SEEDS {
        for batch in batches_for(seed, LodTier::Hub) {
            assert!(batch.vertex_count() > 0, "seed {seed} has an empty batch");
            assert_eq!(
                batch.normals.len(),
                batch.positions.len(),
                "seed {seed}: a normal per vertex is what the shader assumes"
            );
            assert_eq!(
                batch.indices.len() % 3,
                0,
                "seed {seed}: indices are drawn as TRIANGLES"
            );
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|&i| (i as usize) < batch.vertex_count()),
                "seed {seed}: an index points past the end of its vertex buffer"
            );
            assert!(
                batch.positions.iter().all(|v| v.is_finite())
                    && batch.normals.iter().all(|v| v.is_finite()),
                "seed {seed}: a NaN would make the whole batch vanish"
            );
            assert!(
                batch.color.iter().all(|c| (0.0..=1.0).contains(c)),
                "seed {seed}: colours reach the shader as 0..=1"
            );
        }
    }
}

/// Smoothing splits vertices on hard edges, so the vertex count may rise —
/// but no triangle may be added or lost on the way to the page.
#[test]
fn smoothing_preserves_the_triangle_count() {
    for seed in SEEDS {
        let (plant, _) = grow(seed, LodTier::Hub).expect("a plant");
        let drawn: usize = batches(&plant)
            .expect("batches")
            .iter()
            .map(Batch::triangle_count)
            .sum();
        assert_eq!(
            drawn,
            plant.triangle_count().expect("a triangle count"),
            "seed {seed}: the page draws a different number of triangles than the model reports"
        );
    }
}

#[test]
fn every_tier_stays_inside_its_budget() {
    for (index, tier) in [LodTier::Hub, LodTier::Distant, LodTier::Icon]
        .into_iter()
        .enumerate()
    {
        assert_eq!(lod_from_index(index as u32), tier);
        for seed in SEEDS {
            let drawn: usize = batches_for(seed, tier)
                .iter()
                .map(Batch::triangle_count)
                .sum();
            assert!(
                drawn <= tier.triangle_budget(),
                "seed {seed} at {tier:?}: {drawn} triangles exceeds {}",
                tier.triangle_budget()
            );
        }
    }
}

/// The whole point of a seed box: typing the same number twice gives the same
/// plant twice.
#[test]
fn a_seed_is_reproducible() {
    for seed in SEEDS {
        assert_eq!(
            mesh_payload(&batches_for(seed, LodTier::Hub)),
            mesh_payload(&batches_for(seed, LodTier::Hub)),
            "seed {seed} is not deterministic"
        );
    }
}

/// And the point of the regenerate button: a different number gives a
/// different plant.
#[test]
fn different_seeds_give_different_plants() {
    let payloads: Vec<_> = SEEDS
        .iter()
        .map(|&seed| mesh_payload(&batches_for(seed, LodTier::Hub)))
        .collect();
    for (i, a) in payloads.iter().enumerate() {
        for (j, b) in payloads.iter().enumerate().skip(i + 1) {
            assert_ne!(
                a, b,
                "seeds {} and {} grew the same plant",
                SEEDS[i], SEEDS[j]
            );
        }
    }
}

/// Every key the page reads, present and sane. A rename here is a blank field
/// on the page, which is exactly the failure this catches.
#[test]
fn the_info_json_carries_what_the_page_reads() {
    let seed = 42;
    let (plant, genotype) = grow(seed, LodTier::Hub).expect("a plant");
    let batches = batches(&plant).expect("batches");
    let info = plant_info(seed, LodTier::Hub, &plant, &genotype, &batches).expect("info");
    let json: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&info).expect("serialisable")).expect("JSON");

    for key in [
        "seed",
        "lod",
        "triangle_count",
        "vertex_count",
        "batch_count",
        "triangle_budget",
        "height",
        "surface_area",
        "volume",
        "iterations",
        "segment_count",
        "leaf_count",
        "petal_count",
        "fruit_count",
        "symbol_count",
        "branch_angle",
        "branch_length",
        "branch_thickness",
        "branching_factor",
        "taper_curve",
        "tropism_elasticity",
        "axis_curvature",
        "cross_section_index",
        "leaf_mesh_index",
        "leaf_scale",
        "leaves_per_segment",
        "produces_flowers",
        "produces_fruit",
        "leaf_color",
        "petal_color",
        "fruit_color",
        "alchemy_effects",
    ] {
        assert!(
            json.get(key).is_some(),
            "the page reads `{key}` and it is missing"
        );
    }

    assert_eq!(json["seed"], seed);
    assert_eq!(json["lod"], "Hub");
    assert!(info.height > 0.0, "a plant with no height would not frame");
    assert!(info.surface_area > 0.0);
    assert!(!info.alchemy_effects.is_empty());
    for hex in [&info.leaf_color, &info.petal_color, &info.fruit_color] {
        assert!(
            hex.len() == 7
                && hex.starts_with('#')
                && hex[1..].chars().all(|c| c.is_ascii_hexdigit()),
            "`{hex}` is not a CSS hex colour"
        );
    }
}

/// An out-of-range tier index is the full-quality plant, not a panic: the page
/// sends whatever is in its select, and a stale one must not break it.
#[test]
fn an_unknown_tier_index_falls_back_to_hub() {
    assert_eq!(lod_from_index(7), LodTier::Hub);
    assert_eq!(lod_from_index(u32::MAX), LodTier::Hub);
}
