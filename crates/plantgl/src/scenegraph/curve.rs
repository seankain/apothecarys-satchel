//! Curves and patches.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/scenegraph/geometry/{curve,
//! beziercurve,nurbscurve,bezierpatch,nurbspatch}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! **Nothing here is translated yet.** These placeholders fix the shape of
//! [`crate::scenegraph::Geometry`]; the real Bézier and NURBS evaluation lands
//! in Phase C (#19). Visitors report them as [`crate::Error::Unsupported`].

macro_rules! curve_stub {
    ($name:ident, $upstream:literal) => {
        #[doc = concat!("Stub for upstream's `", $upstream, "`. Ported in Phase C (#19).")]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[non_exhaustive]
        pub struct $name;
    };
}

curve_stub!(BezierCurve, "BezierCurve");
curve_stub!(NurbsCurve, "NurbsCurve");
curve_stub!(BezierPatch, "BezierPatch");
curve_stub!(NurbsPatch, "NurbsPatch");
