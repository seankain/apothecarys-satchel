//! Compatibility facade over [`crate::interpret`].
//!
//! Plant geometry used to be built here: `PlantMeshData` held hand-rolled
//! stem rings, marker instances for the organs and a hand-written OBJ writer.
//! #21 moved all of it onto `plantgl` — the sweeps, the surfaces and the codec
//! — and this module is what is left: the entry point keeps the name the rest
//! of the workspace calls it by.
//!
//! New code should use [`crate::interpret`] directly.

pub use crate::interpret::{
    generate_from_phenotype, generate_plant_at, generate_plant_mesh, PlantMaterials, PlantModel,
    PlantStats,
};
