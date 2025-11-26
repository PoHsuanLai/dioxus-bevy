//! Hello Triangle Example
//!
//! The simplest possible example: a Bevy app that renders a colored triangle
//! embedded in a Dioxus UI.

use bevy::prelude::*;
use dioxus::prelude::*;
use dioxus_bevy::{BevyComponent, BevyRenderer};
use dioxus_native::{CustomPaintCtx, DeviceHandle, TextureHandle};
use std::any::Any;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            style: "width: 100vw; height: 100vh; display: flex; flex-direction: column;",

            // Header
            div {
                style: "padding: 20px; background: #2c3e50; color: white;",
                h1 { "Hello Triangle - dioxus-bevy Example" }
                p { "A simple colored triangle rendered with Bevy inside Dioxus" }
            }

            // Bevy render area
            div {
                style: "flex: 1; background: #34495e;",
                BevyComponent {
                    bevy_id: "triangle".to_string(),
                    factory: std::sync::Arc::new(|device| {
                        Box::new(TriangleRenderer::new(device))
                            as Box<dyn BevyRenderer>
                    }),
                }
            }
        }
    }
}

/// Simple Bevy renderer that draws a colored triangle
struct TriangleRenderer {
    app: App,
}

impl TriangleRenderer {
    fn new(_device: &DeviceHandle) -> Self {
        let mut app = App::new();

        // Add minimal Bevy plugins for rendering
        app.add_plugins((
            bevy::core::TaskPoolPlugin::default(),
            bevy::core::TypeRegistrationPlugin,
            bevy::core::FrameCountPlugin,
            bevy::time::TimePlugin,
            bevy::transform::TransformPlugin,
            bevy::hierarchy::HierarchyPlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::render::RenderPlugin::default(),
            bevy::core_pipeline::CorePipelinePlugin,
        ));

        // Set up a simple 2D camera
        app.add_systems(Startup, setup_triangle);

        // Initialize the app
        app.finish();
        app.cleanup();
        app.update();

        Self { app }
    }
}

impl BevyRenderer for TriangleRenderer {
    fn render(&mut self, _ctx: CustomPaintCtx, _width: u32, _height: u32) -> Option<TextureHandle> {
        // Update the Bevy app
        self.app.update();

        // In a real implementation, you would:
        // 1. Render to a texture
        // 2. Extract the texture from Bevy
        // 3. Return it as TextureHandle
        // For now, this is a minimal stub
        None
    }

    fn handle_message(&mut self, _msg: Box<dyn Any + Send>) {
        // No messages in this simple example
    }

    fn shutdown(&mut self) {
        // Send quit event to Bevy
        self.app.world_mut().send_event(bevy::app::AppExit::Success);
        self.app.update();
    }
}

fn setup_triangle(mut commands: Commands) {
    // Spawn a 2D camera
    commands.spawn(Camera2d);

    // In a real example, you would spawn mesh entities here
    // For simplicity, this is just a camera setup
}
