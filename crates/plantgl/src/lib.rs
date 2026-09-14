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
//! Phases A, B and C of the port:
//!
//! - **A** — the math layer, explicit (vertex-list) geometry, appearances,
//!   scenes, transformations, transform accumulation and OBJ export.
//! - **B** — the parametric primitives ([`scenegraph::primitive`]) and the
//!   pipeline over them: [`algo::discretize`] samples them to explicit meshes,
//!   [`algo::tessellate`] reduces those to triangles, [`algo::merge`] batches
//!   them by appearance, and [`algo::measure`], [`algo::bbox`] and
//!   [`algo::bsphere`] read them.
//! - **C** — Bézier and NURBS curves and patches ([`scenegraph::curve`]) and
//!   the generalized cylinder, [`Extrusion`], which sweeps a 2D cross-section
//!   along a 3D axis under rotation-minimising frames.
//! - **D** — the 3D turtle ([`modelling`]): the cpfg/L-studio command set,
//!   generalized cylinders, polygons, cross-sections, guides and tropism, over
//!   three drawers — a [`Scene`], one merged [`TriangleSet`] per appearance, or
//!   nothing but metrics.
//!
//! The L-system driver that feeds the turtle lives in `crates/botany`; wiring
//! it up is Phase E. See `docs/design/08-plantgl-port.md`.
//!
//! A plant, in the shape an L-system driver would produce it: one swept axis
//! per branch, a leaf hung off each.
//!
//! ```
//! use plantgl::modelling::{MeasureDrawer, Turtle};
//!
//! let mut turtle = Turtle::upright(MeasureDrawer::new());
//! turtle.set_tropism(-plantgl::math::Vec3::y()); // gravity, in a Y-up world
//! turtle.set_elasticity(0.2);
//! turtle.set_width(0.05).unwrap();
//!
//! turtle.start_gc();
//! for _ in 0..8 {
//!     turtle.forward_tapered(0.15, turtle.width() * 0.92).unwrap();
//!     turtle.roll_left(45.0);
//! }
//! turtle.stop_gc().unwrap();
//!
//! for _ in 0..3 {
//!     turtle.push();
//!     turtle.left(40.0);
//!     turtle.surface("l", 0.3).unwrap();
//!     turtle.pop().unwrap();
//!     turtle.roll_left(120.0);
//! }
//!
//! // Harvest yield, without ever building a mesh.
//! let measures = turtle.drawer().measures();
//! assert_eq!(measures.segment_count, 8);
//! assert!(measures.surface_area > 0.0 && measures.volume > 0.0);
//! ```
//!
//! ```
//! use plantgl::algo::{discretize, measure, tessellate};
//! use plantgl::scenegraph::{Cylinder, Geometry};
//!
//! let stem = Geometry::from(Cylinder::sized(0.05, 1.0));
//! let mesh = discretize::discretize(&stem).unwrap();
//!
//! // Surface area is a gameplay input as much as a rendering one: it is a
//! // principled harvest-yield driver tied to the visible phenotype.
//! let area = measure::surface_area(&mesh).unwrap();
//! assert!((area - (0.05f32 * 1.0 * std::f32::consts::TAU
//!     + 2.0 * std::f32::consts::PI * 0.05 * 0.05)).abs() < 0.01);
//!
//! let triangles = tessellate::tessellate(&mesh).unwrap();
//! assert_eq!(triangles.face_count(), 32);
//! ```
//!
//! A stem: a circular cross-section swept along a curved axis, narrowing as it
//! goes. This is the shape `Cylinder` cannot make.
//!
//! ```
//! use plantgl::algo::{discretize, measure};
//! use plantgl::math::Point3;
//! use plantgl::scenegraph::mesh::Polyline;
//! use plantgl::{Curve2D, Curve3D, Extrusion, Geometry, NurbsCurve2D, QuantisedFunction};
//!
//! // The axis leans over as it rises, the way a laden stem does.
//! let axis = Curve3D::from(Polyline::new(vec![
//!     Point3::new(0.0, 0.0, 0.0),
//!     Point3::new(0.0, 0.05, 0.4),
//!     Point3::new(0.0, 0.20, 0.8),
//!     Point3::new(0.0, 0.45, 1.1),
//! ]));
//! // An exact circle, not an n-gon: the sweep decides its own facet count.
//! let section = Curve2D::from(NurbsCurve2D::circle(0.04).with_stride(12));
//!
//! let stem = Extrusion::with_radius_profile(
//!     axis.into_ref(),
//!     section.into_ref(),
//!     &QuantisedFunction::ramp(1.0, 0.35), // thick at the base, thin at the tip
//!     8,
//! )
//! .with_solid(true);
//!
//! let mesh = discretize::discretize(&Geometry::from(stem)).unwrap();
//! assert!(mesh.is_solid());
//! // A tapering tube of mean radius ~0.027 over an axis ~1.2 long.
//! let volume = measure::volume(&mesh).unwrap();
//! assert!((0.001..0.004).contains(&volume), "{volume}");
//! ```
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
pub mod modelling;
pub mod scenegraph;

pub use algo::{
    bounding_box, bounding_sphere, discretize, merge_scene, surface_area, tessellate, volume,
    BoundingBox, BoundingSphere, DiscretizeCtx, Discretizer, Explicit,
};
pub use error::{Error, Result};
pub use modelling::{
    MeasureDrawer, MeshDrawer, SceneDrawer, SurfaceLibrary, Turtle, TurtleDrawer, TurtleState,
};
pub use math::Frame;
pub use scenegraph::{
    Appearance, AppearanceRef, BezierCurve, BezierCurve2D, BezierPatch, Box3, Color3, Color4, Cone,
    CtrlPointMatrix, Curve2D, Curve3D, Cylinder, Disc, ElevationGrid, Extrusion, Frustum, Geometry,
    GeometryRef, GeometryVisitor, Group, Material, NurbsCurve, NurbsCurve2D, NurbsPatch,
    Paraboloid, ParametricCurve, Polyline2D, QuantisedFunction, Revolution, Scene, Shape, Sphere,
    Swung, TriangleSet,
};
