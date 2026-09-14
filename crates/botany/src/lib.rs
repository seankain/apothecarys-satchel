//! Plant genetics, phenotype expression, L-systems and turtle interpretation.
//!
//! MIT. The geometry this crate drives lives in `crates/plantgl`, which is a
//! translation of [openalea/plantgl](https://github.com/openalea/plantgl) and
//! is licensed **CeCILL-C**. Linking it makes this crate Derivative Software
//! under CeCILL-C Article 5.3.3, which may be distributed under another
//! licence provided the Article 6.4 notice travels with it — see
//! `THIRD-PARTY-LICENSES` at the repository root. Translated PlantGL code must
//! not be copied into this crate; that is the whole point of the boundary.
//!
//! The pipeline runs:
//!
//! ```text
//! PlantGenotype  →  PlantPhenotype  →  LSystem  →  [LSymbol]  →  plantgl::Scene
//!   genetics.rs       phenotype.rs     lsystem.rs             interpret.rs
//! ```
//!
//! ```
//! use apothecarys_botany::genetics::PlantGenotype;
//! use apothecarys_botany::interpret::generate_plant_mesh;
//! use rand::SeedableRng;
//!
//! let mut rng = rand::rngs::StdRng::seed_from_u64(42);
//! let genotype = PlantGenotype::random_wild(&mut rng);
//! let plant = generate_plant_mesh(&genotype, &mut rng).unwrap();
//!
//! // Surface area is a gameplay input as much as a rendering one.
//! assert!(plant.surface_area().unwrap() > 0.0);
//! assert!(plant.triangle_count().unwrap() <= plant.lod().triangle_budget());
//! ```

pub mod genetics;
pub mod interpret;
pub mod lod;
pub mod lsystem;
pub mod mesh_gen;
pub mod phenotype;
pub mod stat_mapping;
pub mod surfaces;

#[cfg(feature = "fyrox")]
pub mod fyrox_bridge;
