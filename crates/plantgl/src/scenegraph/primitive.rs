//! Parametric primitives.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/scenegraph/geometry/{box,sphere,
//! cone,cylinder,frustum,disc,paraboloid,revolution,swung,sor,extrusion,
//! elevationgrid}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! **Nothing here is translated yet.** These are placeholders that fix the
//! shape of [`crate::scenegraph::Geometry`] so later phases add fields rather
//! than variants: the parametric primitives and their discretisation land in
//! Phase B (#18), `Extrusion` in Phase C (#19). Visitors report them as
//! [`crate::Error::Unsupported`].
//!
//! Each is `#[non_exhaustive]`, so no caller outside the crate can build one
//! and then be broken when the real fields arrive.

macro_rules! primitive_stub {
    ($(#[$meta:meta])* $name:ident, $upstream:literal, $phase:literal) => {
        $(#[$meta])*
        #[doc = concat!("Stub for upstream's `", $upstream, "`. Ported in Phase ", $phase, ".")]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[non_exhaustive]
        pub struct $name;
    };
}

primitive_stub!(Box3, "Box", "B (#18)");
primitive_stub!(Sphere, "Sphere", "B (#18)");
primitive_stub!(Cone, "Cone", "B (#18)");
primitive_stub!(Cylinder, "Cylinder", "B (#18)");
primitive_stub!(Frustum, "Frustum", "B (#18)");
primitive_stub!(Disc, "Disc", "B (#18)");
primitive_stub!(Paraboloid, "Paraboloid", "B (#18)");
primitive_stub!(Revolution, "Revolution", "B (#18)");
primitive_stub!(Swung, "Swung", "B (#18)");
primitive_stub!(ElevationGrid, "ElevationGrid", "B (#18)");
primitive_stub!(Extrusion, "Extrusion", "C (#19)");
