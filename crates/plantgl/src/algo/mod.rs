//! Algorithms over the scene graph.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/algo/base/`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189. Individual modules carry their
//! own provenance headers.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream's discretizer, tesselator and measurement computers arrive with
//! the parametric primitives in Phase B (#18).

pub mod bbox;
pub mod matrix;

pub use bbox::{bounding_box, BBoxComputer, BoundingBox};
pub use matrix::{MatrixComputer, Placement};
