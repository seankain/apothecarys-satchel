//! Import and export.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/algo/codec/`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189, of which only the formats the
//! game and its tooling use are in scope. The legacy codecs — VRML, X3D,
//! POV-Ray, Geomview, VGStar, LIG, DTA, AMAP — are not ported.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! PLY lands in Phase F (#22).

pub mod obj;

pub use obj::{from_obj, to_obj, to_obj_with, ObjFiles, ObjOptions};
