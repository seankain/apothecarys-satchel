//! A Rust port of PlantGL's geometry and turtle-modelling layers.
//!
//! Ported from [openalea/plantgl](https://github.com/openalea/plantgl)
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189. Individual modules carry a
//! header naming the upstream file each derives from.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This crate is a translation of that work
//! and is likewise licensed CeCILL-C — **not** MIT, unlike the rest of this
//! workspace. See `crates/plantgl/LICENSE` for the Agreement and
//! `THIRD-PARTY-LICENSES` at the repository root for the notice that
//! accompanies distribution.
//!
//! Work using PlantGL is asked to cite: Pradal C., Boudon F., Nouguier C.,
//! Chopard J., Godin C. 2009. *PlantGL: A python-based geometric library for
//! 3D plant modelling at different scales.* Graphical Models, 71: 1–21.
//!
//! # What is here today
//!
//! Phase A of the port: the math layer, the explicit (vertex-list) geometry,
//! appearances, scenes, transformations, transform accumulation, bounding
//! boxes and OBJ export. Parametric primitives, curves and the turtle are
//! stubbed variants of [`Geometry`] that visitors report as
//! [`Error::Unsupported`]; see `docs/design/08-plantgl-port.md`.
//!
//! ```
//! use plantgl::{Geometry, Scene, Shape, TriangleSet};
//! use plantgl::math::Point3;
//!
//! let triangle = TriangleSet::new(
//!     vec![
//!         Point3::new(0.0, 0.0, 0.0),
//!         Point3::new(1.0, 0.0, 0.0),
//!         Point3::new(0.0, 1.0, 0.0),
//!     ],
//!     vec![[0, 1, 2]],
//! );
//! let scene = Scene::from_shapes(vec![Shape::new(Geometry::from(triangle).into_ref())]);
//!
//! let bbox = scene.bbox().unwrap().unwrap();
//! assert_eq!(bbox.upper_right, Point3::new(1.0, 1.0, 0.0));
//!
//! let files = plantgl::codec::to_obj(&scene).unwrap();
//! assert!(files.obj.contains("f 1//1 2//2 3//3"));
//! ```

#![forbid(unsafe_code)]

pub mod algo;
pub mod codec;
pub mod error;
pub mod math;
pub mod scenegraph;

pub use error::{Error, Result};
pub use math::Frame;
pub use scenegraph::{
    Appearance, AppearanceRef, Color3, Color4, Geometry, GeometryRef, GeometryVisitor, Group,
    Material, Scene, Shape, TriangleSet,
};
