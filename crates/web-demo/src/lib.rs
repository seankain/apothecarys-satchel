//! The browser demo's entry point: a seed goes in, a drawable plant comes out.
//!
//! MIT, like the rest of the workspace outside `crates/plantgl`. This crate
//! links `plantgl` (CeCILL-C) the same way `botany` does and is therefore
//! Derivative Software under CeCILL-C Article 5.3.3; the Article 6.4 notice
//! travels in `THIRD-PARTY-LICENSES`, which `web/build.sh` copies next to the
//! published page. No translated PlantGL code is pasted in here.
//!
//! # Why a hand-rolled C ABI
//!
//! There is no wasm-bindgen. The module exports plain `extern "C"` functions
//! and two byte buffers, so `web/main.js` instantiates the `.wasm` directly
//! and the whole GitLab Pages build is `cargo build --target
//! wasm32-unknown-unknown` — no `wasm-bindgen-cli`, no `wasm-pack`, no
//! generated glue to keep in step with the Rust.
//!
//! The cost is that the boundary is bytes, so it is specified here and decoded
//! in two places. [`mesh_payload`] writes the layout, [`decode`] reads it back
//! in `tests/payload.rs`, and `web/main.js` reads it again into WebGL buffers.
//! The tests are what keep those two readers honest.
//!
//! # Calling it
//!
//! ```text
//! plant_generate(seed_lo, seed_hi, lod) -> 0 on success, 1 on failure
//! plant_mesh_ptr() / plant_mesh_len()   -> the binary mesh (empty on failure)
//! plant_info_ptr() / plant_info_len()   -> UTF-8 JSON metadata, or {"error":…}
//! ```
//!
//! The results are retained in the module rather than returned, which is what
//! lets the caller copy them out without ever allocating or freeing across the
//! boundary.

use apothecarys_botany::genetics::PlantGenotype;
use apothecarys_botany::interpret::{generate_plant_at, PlantModel};
use apothecarys_botany::lod::LodTier;
use apothecarys_botany::stat_mapping::genetics_to_effects;
use plantgl::algo::normals::{smooth_normals, DEFAULT_CREASE_ANGLE};
use plantgl::scenegraph::Appearance;
use plantgl::Result;
use rand::SeedableRng;
use serde::Serialize;
use std::cell::RefCell;

/// `getrandom`'s custom backend. Unreachable in practice: the only RNG this
/// crate builds is seeded, so nothing ever asks the OS for entropy. It exists
/// to satisfy the linker, and fails loudly rather than returning zeros if that
/// ever stops being true.
#[cfg(target_arch = "wasm32")]
fn unsupported_entropy(_buf: &mut [u8]) -> std::result::Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}

#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(unsupported_entropy);

/// Tags the payload so a stale `.wasm` served from a cache is a clear error in
/// the console rather than a garbled plant. "PLNT" little-endian.
pub const MAGIC: u32 = 0x544E_4C50;
/// Bumped whenever the layout below changes.
pub const VERSION: u32 = 1;

/// Bytes before the first batch header: magic, version, batch count, padding.
const PRELUDE_BYTES: usize = 16;
/// Bytes per batch header: five `u32` fields and an RGBA colour.
const BATCH_HEADER_BYTES: usize = 9 * 4;

thread_local! {
    /// The last payload built, held so the caller can copy it out.
    static LAST: RefCell<Payload> = const { RefCell::new(Payload::empty()) };
}

/// What one [`plant_generate`] call left behind.
struct Payload {
    mesh: Vec<u8>,
    info: String,
}

impl Payload {
    const fn empty() -> Self {
        Self {
            mesh: Vec::new(),
            info: String::new(),
        }
    }
}

/// One draw call's worth of geometry: the positions, normals and indices of
/// every shape that shared an appearance, and the colour to draw them in.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// Vertex positions, `3 * vertex_count` floats.
    pub positions: Vec<f32>,
    /// Vertex normals, one per position, smoothed up to
    /// [`DEFAULT_CREASE_ANGLE`].
    pub normals: Vec<f32>,
    /// Triangle corners, `3 * triangle_count` indices into the two above.
    pub indices: Vec<u32>,
    /// The material's diffuse colour and opacity, 0..=1.
    pub color: [f32; 4],
}

impl Batch {
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// The metadata the page shows beside the plant.
///
/// Field names are the JSON keys `web/main.js` reads; renaming one is a
/// breaking change to the page.
#[derive(Debug, Clone, Serialize)]
pub struct PlantInfo {
    pub seed: u64,
    pub lod: &'static str,
    pub triangle_count: usize,
    pub vertex_count: usize,
    pub batch_count: usize,
    pub triangle_budget: usize,
    /// World units, tip to base.
    pub height: f32,
    pub surface_area: f32,
    pub volume: f32,
    pub iterations: u32,
    pub segment_count: usize,
    pub leaf_count: usize,
    pub petal_count: usize,
    pub fruit_count: usize,
    pub symbol_count: usize,
    pub branch_angle: f32,
    pub branch_length: f32,
    pub branch_thickness: f32,
    pub branching_factor: u32,
    pub taper_curve: String,
    pub tropism_elasticity: f32,
    pub axis_curvature: f32,
    pub cross_section_index: usize,
    pub leaf_mesh_index: usize,
    pub leaf_scale: f32,
    pub leaves_per_segment: u32,
    pub produces_flowers: bool,
    pub produces_fruit: bool,
    /// Hex triples, for the swatches on the page.
    pub leaf_color: String,
    pub petal_color: String,
    pub fruit_color: String,
    /// `AlchemyEffect`s, rendered as short human-readable lines.
    pub alchemy_effects: Vec<String>,
}

/// The tier a `lod` argument selects. Anything out of range is
/// [`LodTier::Hub`], so a caller that guesses gets the full-quality plant.
pub fn lod_from_index(lod: u32) -> LodTier {
    match lod {
        1 => LodTier::Distant,
        2 => LodTier::Icon,
        _ => LodTier::Hub,
    }
}

fn lod_name(lod: LodTier) -> &'static str {
    match lod {
        LodTier::Hub => "Hub",
        LodTier::Distant => "Distant",
        LodTier::Icon => "Icon",
    }
}

/// `PlantPreviewData::from_seed_at`'s pipeline, without the tools crate's
/// optional engine dependency: seed → genotype → phenotype → plant.
pub fn grow(seed: u64, lod: LodTier) -> Result<(PlantModel, PlantGenotype)> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let genotype = PlantGenotype::random_wild(&mut rng);
    let plant = generate_plant_at(&genotype, &mut rng, lod)?;
    Ok((plant, genotype))
}

/// The plant's merged batches, with normals a renderer can light.
///
/// [`PlantModel::batches`] merges by appearance and tessellates, but leaves
/// the normals to whoever draws the result. Smoothing here — rather than
/// per-face in JS — is what keeps a swept stem round and a leaf's rim sharp,
/// and it costs nothing at these triangle counts.
pub fn batches(plant: &PlantModel) -> Result<Vec<Batch>> {
    plant
        .batches()?
        .into_iter()
        .map(|batch| {
            let mesh = smooth_normals(&batch.mesh, DEFAULT_CREASE_ANGLE)?;
            let mut positions = Vec::with_capacity(mesh.model.points.len() * 3);
            for point in mesh.model.points.iter() {
                positions.extend_from_slice(&[point.x, point.y, point.z]);
            }
            // `smooth_normals` returns one normal per point, sharing the
            // position indices, so the two arrays line up as a GPU wants.
            let mut normals = Vec::with_capacity(positions.len());
            match &mesh.model.normals {
                Some(list) => {
                    for normal in list.iter() {
                        normals.extend_from_slice(&[normal.x, normal.y, normal.z]);
                    }
                }
                None => normals.resize(positions.len(), 0.0),
            }
            let indices = mesh.indices.iter().flatten().copied().collect();
            Ok(Batch {
                positions,
                normals,
                indices,
                color: batch
                    .appearance
                    .as_deref()
                    .map(color_of)
                    .unwrap_or([0.6, 0.6, 0.6, 1.0]),
            })
        })
        .collect()
}

/// A material's diffuse colour as RGBA in 0..=1.
///
/// PlantGL stores an ambient colour and a diffuse *multiplier*, and
/// `botany::interpret` sets the multiplier to one so the stored ambient is the
/// colour itself; `diffuse_color()` is asked for anyway rather than assuming
/// that.
fn color_of(appearance: &Appearance) -> [f32; 4] {
    match appearance {
        Appearance::Material(material) => {
            let diffuse = material.diffuse_color();
            [
                diffuse.red_clamped(),
                diffuse.green_clamped(),
                diffuse.blue_clamped(),
                (1.0 - material.transparency).clamp(0.0, 1.0),
            ]
        }
        // No plant carries a texture today; grey is a visible "unhandled".
        Appearance::Texture2D(_) => [0.6, 0.6, 0.6, 1.0],
    }
}

/// Packs batches into the little-endian buffer `web/main.js` decodes.
///
/// ```text
/// u32 magic, u32 version, u32 batch_count, u32 reserved
/// per batch (in order):
///   u32 vertex_count, u32 index_count,
///   u32 positions_offset, u32 normals_offset, u32 indices_offset,
///   f32 r, f32 g, f32 b, f32 a
/// then each batch's positions, normals and indices, in batch order
/// ```
///
/// Offsets are absolute byte offsets into the buffer, so the reader never has
/// to re-derive where a section starts. Every field is four bytes wide and
/// every section is a whole number of them, which keeps all three typed-array
/// views aligned without padding.
pub fn mesh_payload(batches: &[Batch]) -> Vec<u8> {
    let data_start = PRELUDE_BYTES + BATCH_HEADER_BYTES * batches.len();
    let total: usize = batches
        .iter()
        .map(|b| (b.positions.len() + b.normals.len() + b.indices.len()) * 4)
        .sum();

    let mut out = Vec::with_capacity(data_start + total);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(batches.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    let mut cursor = data_start;
    for batch in batches {
        let positions_offset = cursor;
        cursor += batch.positions.len() * 4;
        let normals_offset = cursor;
        cursor += batch.normals.len() * 4;
        let indices_offset = cursor;
        cursor += batch.indices.len() * 4;

        out.extend_from_slice(&(batch.vertex_count() as u32).to_le_bytes());
        out.extend_from_slice(&(batch.indices.len() as u32).to_le_bytes());
        out.extend_from_slice(&(positions_offset as u32).to_le_bytes());
        out.extend_from_slice(&(normals_offset as u32).to_le_bytes());
        out.extend_from_slice(&(indices_offset as u32).to_le_bytes());
        for channel in batch.color {
            out.extend_from_slice(&channel.to_le_bytes());
        }
    }

    debug_assert_eq!(
        out.len(),
        data_start,
        "batch headers overran the data start"
    );

    for batch in batches {
        for value in &batch.positions {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in &batch.normals {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in &batch.indices {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    out
}

/// Reads [`mesh_payload`] back. The tests' stand-in for `web/main.js`'s
/// decoder, and the reason the layout above stays described by running code
/// rather than only by a comment.
pub fn decode(bytes: &[u8]) -> std::result::Result<Vec<Batch>, String> {
    let u32_at = |offset: usize| -> std::result::Result<u32, String> {
        bytes
            .get(offset..offset + 4)
            .map(|slice| u32::from_le_bytes(slice.try_into().expect("a four-byte slice")))
            .ok_or_else(|| format!("payload ends before offset {offset}"))
    };
    let f32_at =
        |offset: usize| -> std::result::Result<f32, String> { u32_at(offset).map(f32::from_bits) };

    if u32_at(0)? != MAGIC {
        return Err("not a plant payload".to_string());
    }
    if u32_at(4)? != VERSION {
        return Err(format!("payload version {} is not {VERSION}", u32_at(4)?));
    }

    let batch_count = u32_at(8)? as usize;
    let mut batches = Vec::with_capacity(batch_count);
    for index in 0..batch_count {
        let header = PRELUDE_BYTES + BATCH_HEADER_BYTES * index;
        let vertex_count = u32_at(header)? as usize;
        let index_count = u32_at(header + 4)? as usize;
        let positions_offset = u32_at(header + 8)? as usize;
        let normals_offset = u32_at(header + 12)? as usize;
        let indices_offset = u32_at(header + 16)? as usize;
        let color = [
            f32_at(header + 20)?,
            f32_at(header + 24)?,
            f32_at(header + 28)?,
            f32_at(header + 32)?,
        ];

        let read_f32s = |start: usize, count: usize| -> std::result::Result<Vec<f32>, String> {
            (0..count).map(|i| f32_at(start + i * 4)).collect()
        };
        batches.push(Batch {
            positions: read_f32s(positions_offset, vertex_count * 3)?,
            normals: read_f32s(normals_offset, vertex_count * 3)?,
            indices: (0..index_count)
                .map(|i| u32_at(indices_offset + i * 4))
                .collect::<std::result::Result<_, _>>()?,
            color,
        });
    }
    Ok(batches)
}

/// Everything the page needs about a plant it is already drawing.
pub fn plant_info(
    seed: u64,
    lod: LodTier,
    plant: &PlantModel,
    genotype: &PlantGenotype,
    batches: &[Batch],
) -> Result<PlantInfo> {
    let phenotype = &plant.phenotype;
    let hex = |color: apothecarys_botany::phenotype::PlantColor| {
        let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(color.r),
            channel(color.g),
            channel(color.b)
        )
    };

    Ok(PlantInfo {
        seed,
        lod: lod_name(lod),
        triangle_count: batches.iter().map(Batch::triangle_count).sum(),
        vertex_count: batches.iter().map(Batch::vertex_count).sum(),
        batch_count: batches.len(),
        triangle_budget: lod.triangle_budget(),
        height: plant.height()?,
        surface_area: plant.surface_area()?,
        volume: plant.volume()?,
        iterations: phenotype.iterations(),
        segment_count: plant.stats.segment_count,
        leaf_count: plant.stats.leaf_count,
        petal_count: plant.stats.petal_count,
        fruit_count: plant.stats.fruit_count,
        symbol_count: plant.stats.symbol_count,
        branch_angle: phenotype.branch_angle,
        branch_length: phenotype.branch_length,
        branch_thickness: phenotype.branch_thickness,
        branching_factor: phenotype.branching_factor,
        taper_curve: format!("{:?}", phenotype.taper_curve),
        tropism_elasticity: phenotype.tropism_elasticity,
        axis_curvature: phenotype.axis_curvature,
        cross_section_index: phenotype.cross_section_index,
        leaf_mesh_index: phenotype.leaf_mesh_index,
        leaf_scale: phenotype.leaf_scale,
        leaves_per_segment: phenotype.leaves_per_segment,
        produces_flowers: phenotype.produces_flowers,
        produces_fruit: phenotype.produces_fruit,
        leaf_color: hex(phenotype.leaf_color),
        petal_color: hex(phenotype.petal_color),
        fruit_color: hex(phenotype.fruit_color),
        alchemy_effects: genetics_to_effects(genotype)
            .iter()
            .map(describe_effect)
            .collect(),
    })
}

/// An [`AlchemyEffect`](apothecarys_core::items::AlchemyEffect) as one line of
/// prose. The demo shows the hidden alchemy genes because they are half of
/// what a seed decides, and `Debug` prints a struct literal, which reads
/// badly on a page.
fn describe_effect(effect: &apothecarys_core::items::AlchemyEffect) -> String {
    use apothecarys_core::items::AlchemyEffect as E;
    match effect {
        E::Heal { amount } => format!("Heal {amount}"),
        E::Damage {
            amount,
            damage_type,
        } => format!("{damage_type:?} damage {amount}"),
        E::Buff { effect, turns } => format!("{} for {turns} turns", describe_status(effect)),
        E::Cure { cures } => format!("Cures {cures:?}"),
        E::StatBoost {
            attribute,
            amount,
            turns,
        } => format!("{attribute:?} {amount:+} for {turns} turns"),
    }
}

/// The same for a [`StatusEffect`](apothecarys_core::stats::StatusEffect),
/// whose variants carry their magnitude in a field.
fn describe_status(effect: &apothecarys_core::stats::StatusEffect) -> String {
    use apothecarys_core::stats::StatusEffect as S;
    match effect {
        S::AttackUp { amount } => format!("Attack {amount:+}"),
        S::DefenseUp { amount } => format!("Defense {amount:+}"),
        S::Regeneration { hp_per_turn } => format!("Regenerate {hp_per_turn} HP/turn"),
        S::Haste => "Haste".to_string(),
        S::Resistance { damage_type } => format!("{damage_type:?} resistance"),
        S::Poisoned { damage_per_turn } => format!("Poisoned {damage_per_turn}/turn"),
        S::Weakened { attack_penalty } => format!("Weakened -{attack_penalty} attack"),
        S::Slowed => "Slowed".to_string(),
        S::Stunned => "Stunned".to_string(),
        S::Blinded => "Blinded".to_string(),
        S::StatBoost { attribute, amount } => format!("{attribute:?} {amount:+}"),
    }
}

/// Grows a plant and builds both payloads, or reports why it could not.
fn build(seed: u64, lod: LodTier) -> std::result::Result<Payload, String> {
    let (plant, genotype) = grow(seed, lod).map_err(|e| e.to_string())?;
    let batches = batches(&plant).map_err(|e| e.to_string())?;
    let info = plant_info(seed, lod, &plant, &genotype, &batches).map_err(|e| e.to_string())?;
    Ok(Payload {
        mesh: mesh_payload(&batches),
        info: serde_json::to_string(&info).map_err(|e| e.to_string())?,
    })
}

// --- The C ABI the browser calls ------------------------------------------
//
// `seed` crosses as two `u32` halves rather than one `u64`. A `u64` parameter
// is a wasm `i64`, which the JS API only accepts as a `BigInt`; splitting it
// keeps the page working on anything that can run the module at all.

/// Grows the plant for `seed` at `lod`, retaining the result for
/// [`plant_mesh_ptr`] and [`plant_info_ptr`].
///
/// Returns `0` on success. On failure it returns `1`, leaves the mesh empty
/// and puts `{"error": "…"}` in the info buffer.
///
/// # Safety
///
/// None needed by the caller: nothing is passed in or handed back but scalars,
/// and the buffers stay owned by the module.
#[no_mangle]
pub extern "C" fn plant_generate(seed_lo: u32, seed_hi: u32, lod: u32) -> u32 {
    let seed = ((seed_hi as u64) << 32) | seed_lo as u64;
    let (payload, status) = match build(seed, lod_from_index(lod)) {
        Ok(payload) => (payload, 0),
        Err(message) => (
            Payload {
                mesh: Vec::new(),
                info: serde_json::json!({ "error": message }).to_string(),
            },
            1,
        ),
    };
    LAST.with(|last| *last.borrow_mut() = payload);
    status
}

/// Where the last call's mesh buffer starts in the module's memory.
#[no_mangle]
pub extern "C" fn plant_mesh_ptr() -> *const u8 {
    LAST.with(|last| last.borrow().mesh.as_ptr())
}

/// How many bytes of it there are.
#[no_mangle]
pub extern "C" fn plant_mesh_len() -> u32 {
    LAST.with(|last| last.borrow().mesh.len() as u32)
}

/// Where the last call's UTF-8 JSON metadata starts.
#[no_mangle]
pub extern "C" fn plant_info_ptr() -> *const u8 {
    LAST.with(|last| last.borrow().info.as_ptr())
}

/// How many bytes of it there are.
#[no_mangle]
pub extern "C" fn plant_info_len() -> u32 {
    LAST.with(|last| last.borrow().info.len() as u32)
}

/// The payload layout the module writes. The page refuses to draw a mesh
/// whose version it does not know, which is what makes a cached `.wasm` from
/// an older deploy an error message rather than a corrupt plant.
#[no_mangle]
pub extern "C" fn plant_payload_version() -> u32 {
    VERSION
}
