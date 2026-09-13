//! The scene graph: geometry, transformations, appearances and scenes.
//!
//! Corresponds to PlantGL `src/cpp/plantgl/scenegraph/`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189. Individual modules carry their
//! own provenance headers.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. Translations of that work in this crate
//! are likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

pub mod appearance;
pub mod curve;
pub mod function;
pub mod geometry;
pub mod mesh;
pub mod primitive;
pub mod scene;
pub mod transform;

pub use appearance::{
    Appearance, AppearanceRef, Color3, Color4, ImageTexture, Material, Texture2D,
    Texture2DTransformation,
};
pub use curve::{
    BezierCurve, BezierCurve2D, BezierPatch, CtrlPointMatrix, Curve2D, Curve2DRef, Curve3D,
    Curve3DRef, CurvePoint, NurbsCurve, NurbsCurve2D, NurbsPatch, ParametricCurve, Polyline2D,
};
pub use function::QuantisedFunction;
pub use geometry::{Geometry, GeometryRef, GeometryVisitor};
pub use mesh::{
    ExplicitModel, FaceIndex, FaceSet, Group, Index, Index3, Index4, IndexedMesh, PointSet,
    Polyline, QuadSet, TriangleSet,
};
pub use primitive::{
    Box3, Cone, Cylinder, Disc, ElevationGrid, Extrusion, Frustum, HeightField, Paraboloid,
    Revolution, Sphere, Swung,
};
pub use scene::{Scene, Shape, NOID};
pub use transform::{Deformation, Taper, Transform, Transformed};
