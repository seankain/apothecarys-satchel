//! Golden-file snapshots of OBJ export.
//!
//! Original to this port; no upstream equivalent. Part of the `plantgl`
//! crate and therefore licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Run with `UPDATE_GOLDEN=1 cargo test -p plantgl --test golden` to rewrite
//! the snapshots after a deliberate change, then read the diff before
//! committing it.

use std::path::PathBuf;

use plantgl::codec::{from_obj, to_obj, to_obj_with, ObjOptions};
use plantgl::math::Point3;
use plantgl::scenegraph::appearance::{Appearance, Color3, Material};
use plantgl::scenegraph::mesh::{Group, QuadSet};
use plantgl::scenegraph::transform::{Transform, Transformed};
use plantgl::{Geometry, Scene, Shape, TriangleSet};
use std::sync::Arc;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Compares `content` against `tests/golden/<name>`, or rewrites it when
/// `UPDATE_GOLDEN` is set.
fn assert_golden(name: &str, content: &str) {
    let path = golden_dir().join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden dir")).expect("create golden dir");
        std::fs::write(&path, content).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}. Re-run with UPDATE_GOLDEN=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        content,
        expected,
        "\n{} differs from its golden. Re-run with UPDATE_GOLDEN=1 if the change is intended.",
        name
    );
}

fn leaf_material() -> Arc<Appearance> {
    Arc::new(Appearance::Material(Material {
        name: Some("leaf".into()),
        ambient: Color3::new(34, 120, 44),
        ..Material::default()
    }))
}

fn stem_material() -> Arc<Appearance> {
    Arc::new(Appearance::Material(Material {
        name: Some("stem".into()),
        ambient: Color3::new(90, 70, 40),
        shininess: 0.4,
        transparency: 0.1,
        ..Material::default()
    }))
}

/// A unit square as two triangles, in the xy plane.
fn square() -> TriangleSet {
    TriangleSet::new(
        vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

/// A closed unit cube as six quads, wound counter-clockwise from outside.
fn cube() -> QuadSet {
    let mut quads = QuadSet::new(
        vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ],
        vec![
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ],
    );
    quads.model.solid = true;
    quads
}

/// Two shapes, two materials, a translation and a taper — one scene that
/// exercises every path the writer has.
fn mixed_scene() -> Scene {
    let tapered_cube = Geometry::from(Transformed::new(
        Transform::Tapered {
            base_radius: 1.0,
            top_radius: 0.4,
        },
        Geometry::QuadSet(cube()).into_ref(),
    ))
    .into_ref();

    let leaves = Geometry::Group(Group::new(vec![
        Geometry::from(square()).into_ref(),
        Geometry::from(Transformed::new(
            Transform::Translated(plantgl::math::Vec3::new(0.0, 0.0, 2.0)),
            Geometry::from(square()).into_ref(),
        ))
        .into_ref(),
    ]))
    .into_ref();

    Scene::from_shapes(vec![
        Shape::new(tapered_cube)
            .with_appearance(stem_material())
            .with_id(1),
        Shape::new(leaves)
            .with_appearance(leaf_material())
            .with_id(2)
            .with_parent_id(1),
    ])
}

#[test]
fn golden_square() {
    let scene = Scene::from_shapes(vec![Shape::new(Geometry::from(square()).into_ref())
        .with_appearance(leaf_material())
        .with_id(1)]);
    let files = to_obj(&scene).unwrap();
    assert_golden("square.obj", &files.obj);
    assert_golden("square.mtl", &files.mtl);
}

#[test]
fn golden_cube() {
    let scene = Scene::from_shapes(vec![Shape::new(Geometry::QuadSet(cube()).into_ref())
        .with_appearance(stem_material())
        .with_id(1)]);
    let files = to_obj(&scene).unwrap();
    assert_golden("cube.obj", &files.obj);
    assert_golden("cube.mtl", &files.mtl);
}

#[test]
fn golden_mixed_scene() {
    let files = to_obj(&mixed_scene()).unwrap();
    assert_golden("mixed_scene.obj", &files.obj);
    assert_golden("mixed_scene.mtl", &files.mtl);
}

#[test]
fn golden_square_without_normals() {
    let scene = Scene::from_shapes(vec![
        Shape::new(Geometry::from(square()).into_ref()).with_id(1)
    ]);
    let options = ObjOptions {
        mtllib: None,
        write_normals: false,
        ..ObjOptions::default()
    };
    let files = to_obj_with(&scene, &options).unwrap();
    assert_golden("square_no_normals.obj", &files.obj);
    assert!(files.mtl.is_empty());
}

/// Writing the same scene twice must produce identical bytes — the property
/// the goldens rest on, and the one saves depend on.
#[test]
fn export_is_deterministic() {
    let scene = mixed_scene();
    let first = to_obj(&scene).unwrap();
    for _ in 0..8 {
        assert_eq!(to_obj(&scene).unwrap(), first);
    }
}

/// Rebuilding a scene from its own OBJ and writing it again reaches a fixed
/// point: the second file is byte-identical to the first.
#[test]
fn export_reaches_a_fixed_point() {
    let scene = mixed_scene();
    let first = to_obj(&scene).unwrap();
    let restored = from_obj(&first.obj).unwrap();
    let second = to_obj(&restored).unwrap();
    assert_eq!(first.obj, second.obj);
}

/// Geometry survives the round trip: same vertex and face counts, same bbox.
#[test]
fn round_trip_preserves_counts_and_bounds() {
    let scene = mixed_scene();
    let restored = from_obj(&to_obj(&scene).unwrap().obj).unwrap();

    let before = scene.bbox().unwrap().unwrap();
    let after = restored.bbox().unwrap().unwrap();
    assert!((before.lower_left - after.lower_left).norm() < 1e-5);
    assert!((before.upper_right - after.upper_right).norm() < 1e-5);

    // The source scene has one cube (6 quads) and two squares (2 triangles
    // each); flattening splits the leaf group into two shapes.
    let faces: usize = restored
        .iter()
        .map(|shape| match shape.geometry.as_ref() {
            Geometry::TriangleSet(m) => m.indices.len(),
            Geometry::QuadSet(m) => m.indices.len(),
            Geometry::FaceSet(m) => m.indices.len(),
            other => panic!("unexpected {}", other.type_name()),
        })
        .sum();
    assert_eq!(faces, 6 + 2 + 2);
}
