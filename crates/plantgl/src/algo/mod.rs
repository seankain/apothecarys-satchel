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
//! The pipeline runs left to right: a parametric [`Geometry`] is
//! [`discretize`]d to an [`Explicit`] model, [`tessellate`]d to triangles, and
//! [`merge`]d into one batch per appearance. [`bbox`], [`bsphere`] and
//! [`measure`] read whatever comes out; [`matrix`] places it.
//!
//! [`Geometry`]: crate::scenegraph::Geometry
//! [`Explicit`]: discretize::Explicit

pub mod bbox;
pub mod bsphere;
pub mod discretize;
pub mod matrix;
pub mod measure;
pub mod merge;
pub mod normals;
pub mod tessellate;

pub use bbox::{bounding_box, BBoxComputer, BoundingBox};
pub use bsphere::{bounding_sphere, BSphereComputer, BoundingSphere};
pub use discretize::{discretize, discretize_with, DiscretizeCtx, Discretizer, Explicit};
pub use matrix::{MatrixComputer, Placement};
pub use measure::{surface_area, triangle_area, volume};
pub use merge::{merge_explicit, merge_scene, MergedBatch};
pub use normals::{compute_normals, smooth_normals, DEFAULT_CREASE_ANGLE};
pub use tessellate::{tessellate, tessellate_geometry};
