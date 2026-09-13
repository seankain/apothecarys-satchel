//! The generalized cylinder.
//!
//! Corresponds to PlantGL
//! `src/cpp/plantgl/scenegraph/geometry/extrusion.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! **Nothing here is translated yet.** `Extrusion` sweeps a 2D cross-section
//! along a 3D axis curve under rotation-minimising frames, so it needs the
//! Bézier and NURBS evaluation that lands in Phase C (#19). This placeholder
//! keeps the shape of [`crate::scenegraph::Geometry`] fixed until then, and
//! visitors report it as [`crate::Error::Unsupported`].

/// Stub for upstream's `Extrusion`. Ported in Phase C (#19).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Extrusion;
