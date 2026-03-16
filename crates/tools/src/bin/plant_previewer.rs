use std::env;

use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        log::{Log, MessageKind},
        math::Rect,
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    engine::{executor::Executor, GraphicsContextParams},
    event_loop::EventLoop,
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    scene::{
        base::BaseBuilder,
        camera::{CameraBuilder, OrthographicProjection, Projection},
        light::{directional::DirectionalLightBuilder, BaseLightBuilder},
        mesh::{
            surface::{SurfaceBuilder, SurfaceData, SurfaceResource},
            MeshBuilder, RenderPath,
        },
        transform::TransformBuilder,
        Scene,
    },
    window::WindowAttributes,
};
use fyrox::asset::untyped::ResourceKind;

use apothecarys_botany::turtle::Vec3 as BotanyVec3;
use apothecarys_tools::plant_preview::PlantPreviewData;

/// Convert our botany Vec3 to Fyrox Vector3.
fn to_fyrox_vec3(v: BotanyVec3) -> Vector3<f32> {
    Vector3::new(v.x, v.y, v.z)
}

/// Plugin for the plant previewer tool.
#[derive(Default, Visit, Reflect, Debug)]
struct PlantPreviewerPlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    seed: u64,

    #[visit(skip)]
    #[reflect(hidden)]
    scene_handle: Handle<Scene>,
}

impl PlantPreviewerPlugin {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            scene_handle: Handle::NONE,
        }
    }

    fn build_scene(&self, context: &mut PluginContext) -> Handle<Scene> {
        let mut scene = Scene::new();

        // Generate the plant
        let preview = PlantPreviewData::from_seed(self.seed);
        preview.print_summary();

        // Export OBJ to file for external viewing
        let obj_path = format!("plant_seed_{}.obj", self.seed);
        if let Err(e) = std::fs::write(&obj_path, preview.mesh.to_obj()) {
            Log::writeln(
                MessageKind::Warning,
                format!("Failed to write OBJ file: {e}"),
            );
        } else {
            Log::writeln(
                MessageKind::Information,
                format!("Exported OBJ to {obj_path}"),
            );
        }

        // Create camera looking at the plant
        let camera_distance = 8.0;
        let camera_pos = Vector3::new(
            camera_distance * 0.7,
            camera_distance * 0.5,
            camera_distance * 0.7,
        );

        let look_dir = (Vector3::new(0.0, 2.0, 0.0) - camera_pos).normalize();
        let camera_rotation =
            UnitQuaternion::face_towards(&look_dir, &Vector3::new(0.0, 1.0, 0.0));

        CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(camera_pos)
                    .with_local_rotation(camera_rotation)
                    .build(),
            ),
        )
        .with_projection(Projection::Orthographic(OrthographicProjection {
            vertical_size: 6.0,
            z_near: 0.1,
            z_far: 100.0,
        }))
        .with_viewport(Rect::new(0.0, 0.0, 1.0, 1.0))
        .build(&mut scene.graph);

        // Add directional light
        DirectionalLightBuilder::new(BaseLightBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_rotation(UnitQuaternion::face_towards(
                        &Vector3::new(-1.0, -2.0, -1.0).normalize(),
                        &Vector3::new(0.0, 1.0, 0.0),
                    ))
                    .build(),
            ),
        ))
        .build(&mut scene.graph);

        // Build the stem mesh from plant data
        self.build_stem_mesh(&preview, &mut scene);

        // Build leaf/flower/fruit meshes as simple colored geometry
        self.build_organ_meshes(&preview, &mut scene);

        // Build a simple ground plane
        self.build_ground_plane(&mut scene);

        context.scenes.add(scene)
    }

    fn build_stem_mesh(&self, preview: &PlantPreviewData, scene: &mut Scene) {
        let mesh = &preview.mesh;
        if mesh.stem_vertices.is_empty() {
            return;
        }

        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();

        for v in &mesh.stem_vertices {
            positions.push(to_fyrox_vec3(v.position));
            normals.push(to_fyrox_vec3(v.normal));
        }

        for &idx in &mesh.stem_indices {
            indices.push(idx);
        }

        let vertex_data = {
                    let mut data = Vec::new();
                    for i in 0..positions.len() {
                        data.extend_from_slice(&positions[i].x.to_le_bytes());
                        data.extend_from_slice(&positions[i].y.to_le_bytes());
                        data.extend_from_slice(&positions[i].z.to_le_bytes());
                        data.extend_from_slice(&normals[i].x.to_le_bytes());
                        data.extend_from_slice(&normals[i].y.to_le_bytes());
                        data.extend_from_slice(&normals[i].z.to_le_bytes());
                    }
                    data
                };
        let surface_data = SurfaceData::new(
            fyrox::scene::mesh::buffer::VertexBuffer::new_with_layout(
                &[
                    fyrox::scene::mesh::buffer::VertexAttributeDescriptor {
                        usage: fyrox::scene::mesh::buffer::VertexAttributeUsage::Position,
                        data_type: fyrox::scene::mesh::buffer::VertexAttributeDataType::F32,
                        size: 3,
                        divisor: 0,
                        shader_location: 0,
                        normalized: false,
                    },
                    fyrox::scene::mesh::buffer::VertexAttributeDescriptor {
                        usage: fyrox::scene::mesh::buffer::VertexAttributeUsage::Normal,
                        data_type: fyrox::scene::mesh::buffer::VertexAttributeDataType::F32,
                        size: 3,
                        divisor: 0,
                        shader_location: 1,
                        normalized: false,
                    },
                ],
                positions.len(),
                fyrox::scene::mesh::buffer::BytesStorage::new(vertex_data),
            )
            .unwrap(),
            fyrox::scene::mesh::buffer::TriangleBuffer::new(
                indices
                    .chunks(3)
                    .map(|tri| fyrox::core::math::TriangleDefinition([tri[0], tri[1], tri[2]]))
                    .collect(),
            ),
        );

        MeshBuilder::new(BaseBuilder::new())
            .with_surfaces(vec![SurfaceBuilder::new(SurfaceResource::new_ok(
                ResourceKind::Embedded,
                surface_data,
            ))
            .build()])
            .with_render_path(RenderPath::Forward)
            .build(&mut scene.graph);
    }

    fn build_organ_meshes(&self, preview: &PlantPreviewData, scene: &mut Scene) {
        let mesh = &preview.mesh;

        // Build leaf markers as small cubes
        for leaf in &mesh.leaf_instances {
            let color = Color::opaque(
                (leaf.color.r * 255.0) as u8,
                (leaf.color.g * 255.0) as u8,
                (leaf.color.b * 255.0) as u8,
            );
            self.build_box_marker(
                scene,
                to_fyrox_vec3(leaf.position),
                leaf.scale * 0.15,
                color,
            );
        }

        // Build flower markers
        for flower in &mesh.flower_instances {
            let color = Color::opaque(
                (flower.color.r * 255.0) as u8,
                (flower.color.g * 255.0) as u8,
                (flower.color.b * 255.0) as u8,
            );
            self.build_box_marker(
                scene,
                to_fyrox_vec3(flower.position),
                flower.scale * 0.2,
                color,
            );
        }

        // Build fruit markers
        for fruit in &mesh.fruit_instances {
            let color = Color::opaque(
                (fruit.color.r * 255.0) as u8,
                (fruit.color.g * 255.0) as u8,
                (fruit.color.b * 255.0) as u8,
            );
            self.build_box_marker(
                scene,
                to_fyrox_vec3(fruit.position),
                fruit.scale * 0.2,
                color,
            );
        }
    }

    fn build_box_marker(
        &self,
        scene: &mut Scene,
        position: Vector3<f32>,
        half_size: f32,
        _color: Color,
    ) {
        let surface_data = SurfaceData::make_cube(fyrox::core::algebra::Matrix4::new_scaling(
            half_size,
        ));

        MeshBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(position)
                    .build(),
            ),
        )
        .with_surfaces(vec![SurfaceBuilder::new(SurfaceResource::new_ok(
            ResourceKind::Embedded,
            surface_data,
        ))
        .build()])
        .with_render_path(RenderPath::Forward)
        .build(&mut scene.graph);
    }

    fn build_ground_plane(&self, scene: &mut Scene) {
        let surface_data = SurfaceData::make_cube(fyrox::core::algebra::Matrix4::new_nonuniform_scaling(
            &Vector3::new(5.0, 0.02, 5.0),
        ));

        let _ground_color = Color::opaque(80, 120, 60);

        MeshBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, -0.02, 0.0))
                    .build(),
            ),
        )
        .with_surfaces(vec![SurfaceBuilder::new(SurfaceResource::new_ok(
            ResourceKind::Embedded,
            surface_data,
        ))
        .build()])
        .with_render_path(RenderPath::Forward)
        .build(&mut scene.graph);
    }
}

impl Plugin for PlantPreviewerPlugin {
    fn register(&self, _context: PluginRegistrationContext) {
        Log::writeln(
            MessageKind::Information,
            "Plant Previewer tool registered",
        );
    }

    fn init(&mut self, _scene_path: Option<&str>, mut context: PluginContext) {
        Log::writeln(
            MessageKind::Information,
            format!("Generating plant with seed {}...", self.seed),
        );
        self.scene_handle = self.build_scene(&mut context);
    }

    fn update(&mut self, _context: &mut PluginContext) {}
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let seed = if args.len() > 1 {
        args[1].parse::<u64>().unwrap_or_else(|_| {
            eprintln!("Usage: plant_previewer [seed]");
            eprintln!("  seed: integer seed for plant generation (default: random)");
            std::process::exit(1);
        })
    } else {
        // Use current time as default seed
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    };

    println!("Plant Previewer - seed: {seed}");

    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = format!("Plant Previewer - Seed {seed}");
    window_attributes.resizable = true;

    let mut executor = Executor::from_params(
        EventLoop::new().unwrap(),
        GraphicsContextParams {
            window_attributes,
            vsync: true,
            msaa_sample_count: None,
            graphics_server_constructor: Default::default(),
        },
    );

    executor.add_plugin(PlantPreviewerPlugin::new(seed));
    executor.run();
}
