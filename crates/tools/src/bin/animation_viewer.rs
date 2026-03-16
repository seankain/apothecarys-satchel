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
        camera::{CameraBuilder, Projection},
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

use apothecarys_tools::animation_viewer::AnimationViewerState;

/// Plugin for the animation viewer tool.
#[derive(Default, Visit, Reflect, Debug)]
struct AnimationViewerPlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    scene_handle: Handle<Scene>,

    #[visit(skip)]
    #[reflect(hidden)]
    state: Option<AnimationViewerState>,

    #[visit(skip)]
    #[reflect(hidden)]
    model_path: Option<String>,

    // UI handles
    #[visit(skip)]
    #[reflect(hidden)]
    clip_list: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    play_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    stop_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    status_text: Handle<UiNode>,
}

impl AnimationViewerPlugin {
    fn new(model_path: Option<String>) -> Self {
        Self {
            model_path,
            ..Default::default()
        }
    }

    fn build_scene(&self, context: &mut PluginContext) -> Handle<Scene> {
        let mut scene = Scene::new();

        // Orbit camera initial position
        let state = self.state.as_ref().unwrap();
        let cam_pos = state.camera.calculate_position();
        let camera_pos = Vector3::new(cam_pos[0], cam_pos[1], cam_pos[2]);
        let look_target = Vector3::new(
            state.camera.target[0],
            state.camera.target[1],
            state.camera.target[2],
        );
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
        .with_projection(Projection::Perspective(
            fyrox::scene::camera::PerspectiveProjection {
                fov: std::f32::consts::FRAC_PI_4,
                z_near: 0.1,
                z_far: 100.0,
            },
        ))
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

        // Ground plane
        let surface_data = SurfaceData::make_cube(
            fyrox::core::algebra::Matrix4::new_nonuniform_scaling(&Vector3::new(5.0, 0.01, 5.0)),
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

        context.scenes.add(scene)
    }

    fn build_ui(&mut self, context: &mut PluginContext) {
        let ctx = &mut context.user_interfaces.first_mut().build_ctx();

        self.clip_list = ListViewBuilder::new(
            WidgetBuilder::new()
                .with_width(200.0)
                .with_height(250.0),
        )
        .build(ctx);

        self.play_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(80.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Play/Pause")
        .build(ctx);

        self.stop_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(80.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Stop")
        .build(ctx);

        let button_panel = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(self.play_button)
                .with_child(self.stop_button),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        self.status_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(24.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Animation Viewer - No model loaded")
        .with_wrap(WrapMode::NoWrap)
        .build(ctx);

        let content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(self.clip_list)
                .with_child(button_panel)
                .with_child(self.status_text),
        )
        .add_row(Row::stretch())
        .add_row(Row::strict(32.0))
        .add_row(Row::strict(28.0))
        .add_column(Column::stretch())
        .build(ctx);

        WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(220.0)
                .with_height(350.0)
                .with_desired_position(fyrox::core::algebra::Vector2::new(10.0, 10.0)),
        )
        .with_title(WindowTitle::text("Animation Clips"))
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

impl Plugin for AnimationViewerPlugin {
    fn register(&self, _context: PluginRegistrationContext) {
        Log::writeln(
            MessageKind::Information,
            "Animation Viewer tool registered",
        );
    }

    fn init(&mut self, _scene_path: Option<&str>, mut context: PluginContext) {
        let mut state = AnimationViewerState::new();

        if let Some(path) = &self.model_path {
            Log::writeln(
                MessageKind::Information,
                format!("Model path provided: {path} (model loading requires runtime asset manager)"),
            );
            state.model_path = Some(path.clone());
        }

        self.state = Some(state);
        self.scene_handle = self.build_scene(&mut context);
        self.build_ui(&mut context);
    }

    fn update(&mut self, context: &mut PluginContext) {
        if let Some(state) = &mut self.state {
            let dt = context.dt;
            state.update(dt);
        }
    }

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.play_button {
                if let Some(state) = &mut self.state {
                    state.toggle_playback();
                    let status = format!("Playback: {:?}", state.playback_state);
                    self.update_status(context, &status);
                }
            } else if message.destination() == self.stop_button {
                if let Some(state) = &mut self.state {
                    state.stop();
                    self.update_status(context, "Stopped");
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let model_path = args.get(1).cloned();

    println!("Animation Viewer");
    if let Some(path) = &model_path {
        println!("Model: {path}");
    } else {
        println!("Usage: animation_viewer [model.glb]");
    }

    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = "Animation Viewer - Apothecary's Satchel".to_string();
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

    executor.add_plugin(AnimationViewerPlugin::new(model_path));
    executor.run();
}
