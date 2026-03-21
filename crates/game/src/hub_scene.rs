//! Hub scene setup: creates a minimal blockout scene with a ground plane,
//! directional light, and sky-colored ambient so the level can be dressed
//! with primitives later in the Fyrox editor.

use fyrox::{
    asset::untyped::ResourceKind,
    core::{
        algebra::{Matrix4, UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
    },
    scene::{
        base::BaseBuilder,
        light::{directional::DirectionalLightBuilder, BaseLightBuilder},
        mesh::{
            surface::{SurfaceBuilder, SurfaceData, SurfaceResource},
            MeshBuilder, RenderPath,
        },
        node::Node,
        transform::TransformBuilder,
        Scene,
    },
};

/// Build the hub blockout scene: ground plane, directional light, and ambient.
/// Returns the scene (caller adds it to `context.scenes`).
pub fn build_hub_scene() -> Scene {
    let mut scene = Scene::new();

    // -- Ambient / sky colour (light blue) --
    scene.rendering_options.ambient_lighting_color = Color::from_rgba(135, 190, 230, 255);

    // -- Directional light (sun) --
    DirectionalLightBuilder::new(
        BaseLightBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_rotation(UnitQuaternion::face_towards(
                        &Vector3::new(-0.5, -1.0, -0.3).normalize(),
                        &Vector3::new(0.0, 1.0, 0.0),
                    ))
                    .build(),
            ),
        )
        .with_color(Color::from_rgba(255, 248, 230, 255)),
    )
    .build(&mut scene.graph);

    // -- Ground plane (50×50 world units) --
    build_ground_plane(&mut scene);

    scene
}

fn build_ground_plane(scene: &mut Scene) -> Handle<Node> {
    // A flat quad scaled to 50×50, lying on Y=0.
    let surface_data = SurfaceData::make_quad(
        &Matrix4::new_nonuniform_scaling(&Vector3::new(50.0, 1.0, 50.0)),
    );

    MeshBuilder::new(
        BaseBuilder::new()
            .with_name("GroundPlane")
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 0.0, 0.0))
                    // Rotate the XY quad so it lies flat on XZ.
                    .with_local_rotation(UnitQuaternion::from_axis_angle(
                        &Vector3::x_axis(),
                        -std::f32::consts::FRAC_PI_2,
                    ))
                    .build(),
            ),
    )
    .with_surfaces(vec![SurfaceBuilder::new(SurfaceResource::new_ok(
        ResourceKind::Embedded,
        surface_data,
    ))
    .build()])
    .with_render_path(RenderPath::Forward)
    .build(&mut scene.graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fyrox::graph::SceneGraph;

    #[test]
    fn test_build_hub_scene_has_nodes() {
        let scene = build_hub_scene();
        // Root + light + ground plane = at least 3 nodes
        assert!(scene.graph.node_count() >= 3);
    }

    #[test]
    fn test_ground_plane_exists() {
        let scene = build_hub_scene();
        let found = scene
            .graph
            .pair_iter()
            .any(|(_, node): (Handle<Node>, &Node)| node.name() == "GroundPlane");
        assert!(found, "GroundPlane node should exist");
    }

    #[test]
    fn test_ambient_color_set() {
        let scene = build_hub_scene();
        // Should not be pure black (default)
        assert_ne!(
            scene.rendering_options.ambient_lighting_color,
            Color::from_rgba(0, 0, 0, 255)
        );
    }
}
