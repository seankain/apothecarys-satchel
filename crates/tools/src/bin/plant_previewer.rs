//! Renders one generated plant, at a seed, through the Fyrox bridge.
//!
//! ```text
//! cargo run --bin plant_previewer -- <seed> [hub|distant|icon]
//! ```
//!
//! The plant arrives as a `plantgl::Scene` — one swept `Extrusion` per branch
//! axis and one `BezierPatch` per blade — which
//! `apothecarys_botany::fyrox_bridge` merges by appearance and repacks into
//! Fyrox surfaces. OBJ and MTL are written alongside, from the same scene.

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
    material::{Material, MaterialResource},
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

use apothecarys_botany::fyrox_bridge::plant_to_node;
use apothecarys_botany::lod::LodTier;
use apothecarys_tools::plant_preview::PlantPreviewData;

/// Create a Fyrox material with the given diffuse color.
fn colored_material(color: Color) -> MaterialResource {
    let mut material = Material::standard();
    material.set_property("diffuseColor", color);
    MaterialResource::new_ok(ResourceKind::Embedded, material)
}

/// Plugin for the plant previewer tool.
#[derive(Default, Visit, Reflect, Debug)]
struct PlantPreviewerPlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    seed: u64,

    #[visit(skip)]
    #[reflect(hidden)]
    lod: LodTier,

    #[visit(skip)]
    #[reflect(hidden)]
    scene_handle: Handle<Scene>,
}

impl PlantPreviewerPlugin {
    fn new(seed: u64, lod: LodTier) -> Self {
        Self {
            seed,
            lod,
            scene_handle: Handle::NONE,
        }
    }

    fn build_scene(&self, context: &mut PluginContext) -> Handle<Scene> {
        let mut scene = Scene::new();

        // Generate the plant
        let preview = PlantPreviewData::from_seed_at(self.seed, self.lod);
        preview.print_summary();

        self.export(&preview);
        self.build_camera(&preview, &mut scene);
        self.build_light(&mut scene);

        // The plant itself, through the bridge: merged by appearance, so this
        // is a handful of surfaces rather than one per leaf.
        match plant_to_node(&preview.plant, &mut scene.graph) {
            Ok(handle) => Log::writeln(
                MessageKind::Information,
                format!("Built plant node {handle:?}"),
            ),
            Err(e) => Log::writeln(
                MessageKind::Error,
                format!("Failed to build the plant node: {e}"),
            ),
        }

        self.build_ground_plane(&mut scene);

        context.scenes.add(scene)
    }

    /// Export OBJ and MTL files for external viewing.
    fn export(&self, preview: &PlantPreviewData) {
        let mtl_filename = format!("plant_seed_{}.mtl", self.seed);
        let obj_path = format!("plant_seed_{}.obj", self.seed);
        let files = preview.to_obj(&mtl_filename);

        for (path, contents) in [(&obj_path, &files.obj), (&mtl_filename, &files.mtl)] {
            match std::fs::write(path, contents) {
                Ok(()) => Log::writeln(MessageKind::Information, format!("Exported {path}")),
                Err(e) => Log::writeln(
                    MessageKind::Warning,
                    format!("Failed to write {path}: {e}"),
                ),
            }
        }
    }

    /// Frames the camera on the plant's own bounding box, so a tall plant and
    /// a squat one are both in shot.
    fn build_camera(&self, preview: &PlantPreviewData, scene: &mut Scene) {
        let (center, height) = match preview.plant.bbox() {
            Ok(Some(bbox)) => {
                let center = bbox.center();
                let size = bbox.size();
                (
                    Vector3::new(center.x, center.y, center.z),
                    size.x.max(size.y).max(size.z).max(1.0),
                )
            }
            _ => (Vector3::new(0.0, 1.0, 0.0), 4.0),
        };

        let distance = height * 2.5;
        let camera_pos = center + Vector3::new(distance * 0.7, distance * 0.5, distance * 0.7);
        let look_dir = (center - camera_pos).normalize();
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
            vertical_size: height * 0.75,
            z_near: 0.1,
            z_far: 100.0,
        }))
        .with_viewport(Rect::new(0.0, 0.0, 1.0, 1.0))
        .build(&mut scene.graph);
    }

    fn build_light(&self, scene: &mut Scene) {
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
    }

    fn build_ground_plane(&self, scene: &mut Scene) {
        let surface_data = SurfaceData::make_cube(fyrox::core::algebra::Matrix4::new_nonuniform_scaling(
            &Vector3::new(5.0, 0.02, 5.0),
        ));

        let ground_material = colored_material(Color::opaque(80, 120, 60));

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
        .with_material(ground_material)
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

fn usage() -> ! {
    eprintln!("Usage: plant_previewer [seed] [hub|distant|icon]");
    eprintln!("  seed: integer seed for plant generation (default: current time)");
    eprintln!("  tier: level-of-detail tier to build at (default: hub)");
    std::process::exit(1);
}

fn parse_lod(name: &str) -> LodTier {
    match name.to_ascii_lowercase().as_str() {
        "hub" => LodTier::Hub,
        "distant" => LodTier::Distant,
        "icon" => LodTier::Icon,
        _ => usage(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let seed = match args.get(1) {
        Some(arg) => arg.parse::<u64>().unwrap_or_else(|_| usage()),
        // Use current time as default seed
        None => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let lod = args.get(2).map(|a| parse_lod(a)).unwrap_or(LodTier::Hub);

    println!("Plant Previewer - seed: {seed}, tier: {lod:?}");

    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = format!("Plant Previewer - Seed {seed} ({lod:?})");
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

    executor.add_plugin(PlantPreviewerPlugin::new(seed, lod));
    executor.run();
}
