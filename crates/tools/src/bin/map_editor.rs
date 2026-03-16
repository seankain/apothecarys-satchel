use std::env;

use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        log::{Log, MessageKind},
        math::Rect,
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    engine::{executor::Executor, GraphicsContextParams},
    event_loop::EventLoop,
    gui::{
        button::{ButtonBuilder, ButtonMessage},
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        list_view::ListViewBuilder,
        message::{MessageDirection, UiMessage},
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        Orientation, Thickness, UiNode,
    },
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

use apothecarys_tools::map_editor::{snap_to_grid, PlacementData, UndoStack};

/// Plugin for the map editor tool.
#[derive(Default, Visit, Reflect, Debug)]
struct MapEditorPlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    scene_handle: Handle<Scene>,

    #[visit(skip)]
    #[reflect(hidden)]
    placement: Option<PlacementData>,

    #[visit(skip)]
    #[reflect(hidden)]
    undo_stack: Option<UndoStack>,

    #[visit(skip)]
    #[reflect(hidden)]
    grid_size: f32,

    #[visit(skip)]
    #[reflect(hidden)]
    file_path: Option<String>,

    // UI handles
    #[visit(skip)]
    #[reflect(hidden)]
    object_list: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    add_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    remove_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    save_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    status_text: Handle<UiNode>,
}

impl MapEditorPlugin {
    fn new(file_path: Option<String>) -> Self {
        Self {
            grid_size: 0.5,
            file_path,
            ..Default::default()
        }
    }

    fn build_scene(&self, context: &mut PluginContext) -> Handle<Scene> {
        let mut scene = Scene::new();

        // Isometric camera
        let camera_distance = 30.0;
        let camera_pos = Vector3::new(
            camera_distance * 0.7,
            camera_distance * 0.5,
            camera_distance * 0.7,
        );
        let look_target = Vector3::new(0.0, 0.0, 0.0);
        let look_dir = (look_target - camera_pos).normalize();
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
            vertical_size: 15.0,
            z_near: 0.1,
            z_far: 200.0,
        }))
        .with_viewport(Rect::new(0.0, 0.0, 1.0, 1.0))
        .build(&mut scene.graph);

        // Directional light
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

        // Ground grid
        self.build_ground_grid(&mut scene);

        context.scenes.add(scene)
    }

    fn build_ground_grid(&self, scene: &mut Scene) {
        // Build a simple ground plane
        let surface_data = SurfaceData::make_cube(
            fyrox::core::algebra::Matrix4::new_nonuniform_scaling(&Vector3::new(
                20.0, 0.01, 20.0,
            )),
        );

        MeshBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, -0.01, 0.0))
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

    fn build_ui(&mut self, context: &mut PluginContext) {
        let ctx = &mut context.user_interfaces.first_mut().build_ctx();

        // Object list
        self.object_list = ListViewBuilder::new(
            WidgetBuilder::new()
                .with_width(200.0)
                .with_height(300.0),
        )
        .build(ctx);

        // Buttons
        self.add_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(90.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Add Object")
        .build(ctx);

        self.remove_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(90.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Remove")
        .build(ctx);

        self.save_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(90.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Save")
        .build(ctx);

        let button_panel = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(self.add_button)
                .with_child(self.remove_button)
                .with_child(self.save_button),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // Status text
        self.status_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(24.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Map Editor - Ready")
        .with_wrap(WrapMode::NoWrap)
        .build(ctx);

        let content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(self.object_list)
                .with_child(button_panel)
                .with_child(self.status_text),
        )
        .add_row(Row::stretch())
        .add_row(Row::strict(32.0))
        .add_row(Row::strict(28.0))
        .add_column(Column::stretch())
        .build(ctx);

        // Main window
        WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(220.0)
                .with_height(400.0)
                .with_desired_position(fyrox::core::algebra::Vector2::new(10.0, 10.0)),
        )
        .with_title(WindowTitle::text("Scene Objects"))
        .with_content(content)
        .open(false)
        .build(ctx);
    }

    fn update_status(&self, context: &mut PluginContext, message: &str) {
        if self.status_text.is_some() {
            context
                .user_interfaces
                .first_mut()
                .send_message(fyrox::gui::text::TextMessage::text(
                    self.status_text,
                    MessageDirection::ToWidget,
                    message.to_string(),
                ));
        }
    }
}

impl Plugin for MapEditorPlugin {
    fn register(&self, _context: PluginRegistrationContext) {
        Log::writeln(MessageKind::Information, "Map Editor tool registered");
    }

    fn init(&mut self, _scene_path: Option<&str>, mut context: PluginContext) {
        // Load or create placement data
        let placement = if let Some(path) = &self.file_path {
            match PlacementData::load(path) {
                Ok(data) => {
                    Log::writeln(
                        MessageKind::Information,
                        format!("Loaded placement from {path}"),
                    );
                    data
                }
                Err(e) => {
                    Log::writeln(
                        MessageKind::Warning,
                        format!("Could not load {path}: {e}. Creating new."),
                    );
                    PlacementData::new("new_location")
                }
            }
        } else {
            PlacementData::new("new_location")
        };

        self.placement = Some(placement);
        self.undo_stack = Some(UndoStack::new(100));
        self.scene_handle = self.build_scene(&mut context);
        self.build_ui(&mut context);

        Log::writeln(
            MessageKind::Information,
            format!(
                "Map Editor initialized with grid size {}",
                self.grid_size
            ),
        );
    }

    fn update(&mut self, _context: &mut PluginContext) {}

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.add_button {
                if let Some(placement) = &mut self.placement {
                    let pos = snap_to_grid([0.0, 0.0, 0.0], self.grid_size);
                    let id = placement.add_object("placeholder.glb".to_string(), pos);
                    Log::writeln(
                        MessageKind::Information,
                        format!("Added object {id} at ({}, {}, {})", pos[0], pos[1], pos[2]),
                    );
                    self.update_status(context, &format!("Added object {id}"));
                }
            } else if message.destination() == self.remove_button {
                if let Some(placement) = &mut self.placement {
                    if let Some(last) = placement.objects.last().map(|o| o.id) {
                        placement.remove_object(last);
                        Log::writeln(
                            MessageKind::Information,
                            format!("Removed object {last}"),
                        );
                        self.update_status(context, &format!("Removed object {last}"));
                    } else {
                        self.update_status(context, "No objects to remove");
                    }
                }
            } else if message.destination() == self.save_button {
                if let Some(placement) = &self.placement {
                    let path = self
                        .file_path
                        .clone()
                        .unwrap_or_else(|| "scene.placement.ron".to_string());
                    match placement.save(&path) {
                        Ok(()) => {
                            Log::writeln(
                                MessageKind::Information,
                                format!("Saved placement to {path}"),
                            );
                            self.update_status(context, &format!("Saved to {path}"));
                        }
                        Err(e) => {
                            Log::writeln(
                                MessageKind::Error,
                                format!("Save failed: {e}"),
                            );
                            self.update_status(context, "Save failed!");
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).cloned();

    println!("Map Editor");
    if let Some(path) = &file_path {
        println!("Loading: {path}");
    }

    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = "Map Editor - Apothecary's Satchel".to_string();
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

    executor.add_plugin(MapEditorPlugin::new(file_path));
    executor.run();
}
