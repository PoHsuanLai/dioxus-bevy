//! Hello Triangle Example
//!
//! The simplest possible example: a Bevy app that renders a colored triangle
//! embedded in a Dioxus UI.

use bevy::prelude::*;
use dioxus::prelude::*;
use dioxus_bevy::bevy_component;

fn main() {
    dioxus_native::launch_cfg(App, Vec::new(), dioxus_bevy::config());
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            style: "width: 100vw; height: 100vh; display: flex; gap: 20px; padding: 20px; box-sizing: border-box;",

            // Dioxus box
            div {
                style: "flex: 1; display: flex; align-items: center; justify-content: center; background: #3498db; color: white; font-size: 32px; font-weight: bold; border-radius: 8px;",
                "This is from Dioxus"
            }

            // Bevy box
            div {
                style: "flex: 1; border-radius: 8px; overflow: hidden;",
                TriangleScene {}
            }
        }
    }
}

#[bevy_component]
fn triangle_scene(app: &mut App) {
    app.add_systems(Startup, setup_triangle);
}

fn setup_triangle(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    // Background rectangle
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(2000.0, 2000.0))),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.9, 0.3, 0.3)))),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    // Triangle
    commands.spawn((
        Mesh2d(meshes.add(Triangle2d::new(
            Vec2::new(0.0, 200.0),
            Vec2::new(-173.0, -100.0),
            Vec2::new(173.0, -100.0),
        ))),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(1.0, 1.0, 1.0)))),
    ));

    // Text
    commands.spawn((
        Text2d::new("This is from Bevy"),
        TextFont {
            font_size: 60.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, -300.0, 0.0),
    ));
}
