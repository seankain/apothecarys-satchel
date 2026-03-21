//! Main menu UI: title screen with Start and Exit buttons.

use fyrox::{
    core::pool::Handle,
    gui::{
        button::ButtonBuilder,
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        BuildContext, HorizontalAlignment, Orientation, Thickness, UiNode, VerticalAlignment,
    },
};

/// Holds widget handles for the main menu so we can identify button clicks
/// and toggle visibility.
#[derive(Debug)]
pub struct MainMenuState {
    pub root: Handle<UiNode>,
    pub start_button: Handle<UiNode>,
    pub exit_button: Handle<UiNode>,
}

impl MainMenuState {
    /// Build the main menu widget tree and return the state with handles.
    pub fn build(ctx: &mut BuildContext) -> Self {
        let title = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::bottom(40.0))
                .with_horizontal_alignment(HorizontalAlignment::Center),
        )
        .with_text("The Apothecary's Satchel")
        .with_font_size(32.0.into())
        .with_horizontal_text_alignment(HorizontalAlignment::Center)
        .build(ctx);

        let start_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(200.0)
                .with_height(40.0)
                .with_margin(Thickness::bottom(10.0))
                .with_horizontal_alignment(HorizontalAlignment::Center),
        )
        .with_text("Start Game")
        .build(ctx);

        let exit_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(200.0)
                .with_height(40.0)
                .with_horizontal_alignment(HorizontalAlignment::Center),
        )
        .with_text("Exit")
        .build(ctx);

        let root = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_horizontal_alignment(HorizontalAlignment::Center)
                .with_vertical_alignment(VerticalAlignment::Center)
                .with_child(title)
                .with_child(start_button)
                .with_child(exit_button),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        Self {
            root,
            start_button,
            exit_button,
        }
    }

    /// Show the main menu.
    pub fn set_visible(&self, ui: &mut fyrox::gui::UserInterface, visible: bool) {
        ui.send_message(fyrox::gui::widget::WidgetMessage::visibility(
            self.root,
            fyrox::gui::message::MessageDirection::ToWidget,
            visible,
        ));
    }
}
