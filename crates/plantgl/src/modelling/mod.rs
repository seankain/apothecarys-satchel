//! The turtle: the command set that turns an L-system string into geometry.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/algo/modelling/`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189. Individual modules carry their
//! own provenance headers.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! [`Turtle`] holds the state and the commands; a [`TurtleDrawer`] decides
//! what those commands produce. Three are provided:
//!
//! - [`SceneDrawer`] — a [`Scene`](crate::scenegraph::Scene) of parametric
//!   shapes. Inspectable, exportable, re-tessellatable at another density.
//! - [`MeshDrawer`] — one merged triangle set per appearance, skipping the
//!   scene graph entirely. The game path.
//! - [`MeasureDrawer`] — surface area, volume, bounding box and segment
//!   count, allocating nothing. Harvest yield without building a mesh.
//!
//! ```
//! use plantgl::modelling::{SceneDrawer, Turtle};
//!
//! // A stem with two branches, swept as generalized cylinders.
//! let mut turtle = Turtle::upright(SceneDrawer::new());
//! turtle.set_width(0.04).unwrap();
//! turtle.start_gc();
//! turtle.forward(1.0).unwrap();
//! turtle.push();
//! turtle.left(40.0);
//! turtle.forward_tapered(0.6, 0.01).unwrap();
//! turtle.pop().unwrap();
//! turtle.right(40.0);
//! turtle.forward_tapered(0.6, 0.01).unwrap();
//! turtle.stop_gc().unwrap();
//!
//! let scene = turtle.into_drawer().into_scene();
//! assert_eq!(scene.len(), 2); // the branch, and the trunk it grew off
//! ```

pub mod drawer;
pub mod geometry;
pub mod measure_drawer;
pub mod mesh_drawer;
pub mod param;
pub mod path;
pub mod scene_drawer;
pub mod tropism;
pub mod turtle;

pub use drawer::{DrawCtx, IdPair, SurfaceLibrary, TurtleDrawer};
pub use measure_drawer::{MeasureDrawer, Measures};
pub use mesh_drawer::{MeshBatch, MeshDrawer};
pub use param::{DrawParams, TextureState, TurtleDefaults, TurtleState};
pub use path::{Guide2D, Guide3D, GuideOutcome, TurtlePath};
pub use scene_drawer::SceneDrawer;
pub use tropism::{tend_to, Reflection};
pub use turtle::{default_materials, Turtle};
