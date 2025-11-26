//! Interactive 3D Cube Example
//!
//! Demonstrates a Bevy 3D scene embedded in Dioxus with UI controls.
//! Shows message passing between Dioxus UI and Bevy renderer.

use bevy::prelude::*;
use dioxus::prelude::*;
use dioxus_bevy::{BevyComponent, BevyRenderer, use_bevy_message};
use dioxus_native::{CustomPaintCtx, DeviceHandle, TextureHandle};
use std::any::Any;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut rotation_speed = use_signal(|| 1.0f32);
    let send_to_bevy = use_bevy_message("cube-scene");

    // Send rotation speed updates to Bevy
    use_effect(move || {
        let speed = *rotation_speed.read();
        send_to_bevy.send(Box::new(CubeMessage::SetRotationSpeed(speed)));
    });

    rsx! {
        div {
            style: "width: 100vw; height: 100vh; display: flex;",

            // Sidebar with controls
            div {
                style: "width: 300px; padding: 20px; background: #2c3e50; color: white;",

                h1 { "Interactive Cube" }
                p { style: "color: #95a5a6;", "dioxus-bevy example" }

                div { style: "margin-top: 30px;",
                    h3 { "Controls" }

                    label {
                        "Rotation Speed: {rotation_speed:.2}"
                        input {
                            r#type: "range",
                            min: "0",
                            max: "5",
                            step: "0.1",
                            value: "{rotation_speed}",
                            style: "width: 100%; margin-top: 10px;",
                            oninput: move |evt| {
                                if let Ok(val) = evt.value().parse::<f32>() {
                                    rotation_speed.set(val);
                                }
                            }
                        }
                    }

                    button {
                        style: "margin-top: 20px; padding: 10px; width: 100%;",
                        onclick: move |_| rotation_speed.set(1.0),
                        "Reset Speed"
                    }

                    button {
                        style: "margin-top: 10px; padding: 10px; width: 100%;",
                        onclick: move |_| {
                            send_to_bevy.send(Box::new(CubeMessage::ResetRotation));
                        },
                        "Reset Rotation"
                    }
                }

                div { style: "margin-top: 30px; padding: 15px; background: rgba(0,0,0,0.2); border-radius: 5px;",
                    h4 { "About" }
                    p { style: "font-size: 12px; line-height: 1.6;",
                        "This example shows a Bevy-rendered 3D cube embedded in a Dioxus UI. "
                        "The rotation speed is controlled by the Dioxus slider and sent to Bevy "
                        "via the message passing system."
                    }
                }
            }

            // 3D render area
            div {
                style: "flex: 1; background: #34495e;",
                BevyComponent {
                    bevy_id: "cube-scene".to_string(),
                    factory: std::sync::Arc::new(|device| {
                        Box::new(CubeRenderer::new(device))
                            as Box<dyn BevyRenderer>
                    }),
                }
            }
        }
    }
}

/// Messages that can be sent to the Bevy renderer
#[derive(Debug)]
enum CubeMessage {
    SetRotationSpeed(f32),
    ResetRotation,
}

/// Bevy renderer for a rotating 3D cube
struct CubeRenderer {
    app: App,
}

impl CubeRenderer {
    fn new(_device: &DeviceHandle) -> Self {
        let mut app = App::new();

        // Add Bevy plugins
        app.add_plugins((
            bevy::core::TaskPoolPlugin::default(),
            bevy::core::TypeRegistrationPlugin,
            bevy::core::FrameCountPlugin,
            bevy::time::TimePlugin,
            bevy::transform::TransformPlugin,
            bevy::hierarchy::HierarchyPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::render::RenderPlugin::default(),
            bevy::core_pipeline::CorePipelinePlugin,
            bevy::pbr::PbrPlugin::default(),
        ));

        // Set up the 3D scene
        app.add_systems(Startup, setup_cube_scene);
        app.add_systems(Update, rotate_cube);

        // Initialize
        app.finish();
        app.cleanup();
        app.update();

        Self { app }
    }
}

impl BevyRenderer for CubeRenderer {
    fn render(&mut self, _ctx: CustomPaintCtx, _width: u32, _height: u32) -> Option<TextureHandle> {
        // Update the Bevy app
        self.app.update();

        // TODO: Extract rendered texture and return it
        None
    }

    fn handle_message(&mut self, msg: Box<dyn Any + Send>) {
        if let Some(msg) = msg.downcast_ref::<CubeMessage>() {
            match msg {
                CubeMessage::SetRotationSpeed(speed) => {
                    // Update rotation speed in Bevy world
                    if let Some(mut rotation_speed) = self.app.world_mut().get_resource_mut::<RotationSpeed>() {
                        rotation_speed.0 = *speed;
                    }
                }
                CubeMessage::ResetRotation => {
                    // Reset cube rotation
                    let mut query = self.app.world_mut().query::<&mut Transform>();
                    for mut transform in query.iter_mut(self.app.world_mut()) {
                        transform.rotation = Quat::IDENTITY;
                    }
                }
            }
        }
    }

    fn shutdown(&mut self) {
        self.app.world_mut().send_event(bevy::app::AppExit::Success);
        self.app.update();
    }
}

/// Resource to store rotation speed
#[derive(Resource)]
struct RotationSpeed(f32);

fn setup_cube_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Insert rotation speed resource
    commands.insert_resource(RotationSpeed(1.0));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 3.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.5, 0.8),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));
}

fn rotate_cube(
    time: Res<Time>,
    rotation_speed: Res<RotationSpeed>,
    mut query: Query<&mut Transform, With<Mesh3d>>,
) {
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * rotation_speed.0);
        transform.rotate_x(time.delta_secs() * rotation_speed.0 * 0.5);
    }
}
