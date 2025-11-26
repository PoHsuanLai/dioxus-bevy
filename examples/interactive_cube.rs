//! Interactive Cube

use bevy::prelude::*;
use dioxus::prelude::*;
use dioxus_bevy::bevy_component;

fn main() {
    // Test: does dioxus_bevy::config() cause the crash?
    dioxus_native::launch_cfg(App, Vec::new(), dioxus_bevy::config());
}

#[component]
fn App() -> Element {
    let mut rotation_speed = use_signal(|| 1.0f32);

    rsx! {
        div {
            style: "width: 100vw; height: 100vh; display: flex; flex-direction: column;",

            div {
                style: "padding: 20px; background: #2c3e50; color: white; text-align: center;",
                h1 { "Interactive Cube" }
                p { "Click the cube to speed up rotation (max 5.0x)" }
                p { "Current speed: {rotation_speed:.2}x" }
            }

            div {
                style: "flex: 1;",
                onclick: move |_| {
                    // Increase speed on click, cycling back to 0.5 after 5.0
                    let new_speed = rotation_speed() + 0.5;
                    rotation_speed.set(if new_speed > 5.0 { 0.5 } else { new_speed });
                },
                CubeScene {
                    rotation_speed,
                }
            }
        }
    }
}

#[bevy_component]
fn cube_scene(app: &mut App, rotation_speed: ReadSignal<f32>) {
    app.insert_resource(RotationSpeed(1.0));
    app.insert_resource(CubeColorIndex(0));
    app.add_systems(Startup, setup_cube);
    app.add_systems(Update, process_signal_updates);
    app.add_systems(Update, rotate_cube);
    app.add_systems(Update, update_cube_color);
}

#[derive(Resource)]
struct RotationSpeed(f32);

#[derive(Resource)]
struct CubeColorIndex(i32);

#[derive(Component)]
struct RotatingCube;

fn process_signal_updates(
    receiver: Res<dioxus_bevy::SignalReceiver>,
    mut speed: ResMut<RotationSpeed>,
    mut color: ResMut<CubeColorIndex>,
) {
    while let Ok(update) = receiver.receiver.try_recv() {
        match update {
            dioxus_bevy::SignalUpdate::F32(key, value) if key == "rotation_speed" => {
                speed.0 = value;
            }
            dioxus_bevy::SignalUpdate::I32(key, value) if key == "color_index" => {
                color.0 = value;
            }
            _ => {}
        }
    }
}

fn setup_cube(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(Camera3d::default()).insert(Transform::from_xyz(3.0, 2.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            ..default()
        })),
        RotatingCube,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn rotate_cube(
    time: Res<Time>,
    speed: Res<RotationSpeed>,
    mut query: Query<&mut Transform, With<RotatingCube>>,
) {
    for mut transform in &mut query {
        transform.rotation = Quat::from_rotation_y(time.elapsed_secs() * speed.0);
    }
}

fn update_cube_color(
    color_index: Res<CubeColorIndex>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<&MeshMaterial3d<StandardMaterial>, With<RotatingCube>>,
) {
    if !color_index.is_changed() {
        return;
    }

    for material_handle in &query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = match color_index.0 {
                0 => Color::srgb(1.0, 0.0, 0.0), // Red
                1 => Color::srgb(0.0, 1.0, 0.0), // Green
                2 => Color::srgb(0.0, 0.0, 1.0), // Blue
                3 => Color::srgb(1.0, 1.0, 0.0), // Yellow
                _ => Color::srgb(1.0, 0.0, 0.0), // Default red
            };
        }
    }
}
