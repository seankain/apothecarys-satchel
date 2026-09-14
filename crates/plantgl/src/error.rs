//! Error type for the crate.
//!
//! Original to this port, replacing PlantGL's warn-and-continue handling.
//! Upstream reports problems through static error handlers installed on
//! `PglErrorStream` (`src/cpp/plantgl/tool/errormsg.h`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189) and carries on with an invalid
//! object; we return `Result` instead, so a malformed scene cannot reach the
//! renderer.
//!
//! This file is part of a crate that is a translation of PlantGL, which is
//! Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and governed by
//! the CeCILL-C license; it is likewise licensed CeCILL-C. See
//! crates/plantgl/LICENSE.

use std::fmt;

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Geometry that cannot be built or sampled: zero-length axes, collapsed
    /// frames, fewer points than the primitive needs.
    #[error("degenerate geometry: {0}")]
    DegenerateGeometry(String),

    /// A field that must reference another list is out of range, or two
    /// parallel lists disagree in length.
    #[error("invalid index: {0}")]
    InvalidIndex(String),

    /// `pop` with nothing pushed. Upstream warns and continues; the L-system
    /// driver may still choose to ignore this.
    #[error("pop on an empty stack")]
    EmptyStack,

    /// A NURBS knot vector that is not non-decreasing, or whose length does
    /// not match `control points + degree + 1`.
    #[error("bad knot vector: {0}")]
    BadKnotVector(String),

    /// `surface(name, …)` naming a template the turtle's surface library does
    /// not hold. Upstream warns and draws nothing.
    #[error("unknown surface: {0}")]
    UnknownSurface(String),

    /// An operation that is valid in general but not for this geometry — in
    /// particular a `Geometry` variant whose primitive is not yet ported.
    #[error("unsupported operation: {0}")]
    Unsupported(String),

    /// A codec could not write or read what it was given.
    #[error("codec error: {0}")]
    Codec(String),
}

impl Error {
    pub fn degenerate(what: impl fmt::Display) -> Self {
        Error::DegenerateGeometry(what.to_string())
    }

    pub fn invalid_index(what: impl fmt::Display) -> Self {
        Error::InvalidIndex(what.to_string())
    }

    pub fn unsupported(what: impl fmt::Display) -> Self {
        Error::Unsupported(what.to_string())
    }

    pub fn codec(what: impl fmt::Display) -> Self {
        Error::Codec(what.to_string())
    }
}
