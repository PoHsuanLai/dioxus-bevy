//! GLTF Model with Asset Management

use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use dioxus::prelude::*;
use dioxus_bevy::{bevy_component, asset_path};
use std::f32::consts::*;

fn main() {
    dioxus_native::launch_cfg(App, Vec::new(), dioxus_bevy::config());
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            style: "width: 100vw; height: 100vh; display: flex; flex-direction: column;",

            div {
                style: "padding: 20px; background: #2c3e50; color: white; text-align: center;",
                h1 { "GLTF Model" }
            }

            div {
                style: "flex: 1;",
                GltfScene {}
            }
        }
    }
}

#[bevy_component]
fn gltf_scene(app: &mut App) {
    app.insert_resource(DirectionalLightShadowMap { size: 4096 });
    app.add_systems(Startup, setup_scene);
    app.add_systems(Update, animate_light);
}

fn setup_scene(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        EnvironmentMapLight {
            diffuse_map: asset_server.load(asset_path("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2")),
            specular_map: asset_server.load(asset_path("environment_maps/pisa_specular_rgb9e5_zstd.ktx2")),
            intensity: 250.0,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 1.6,
            ..default()
        }
        .build(),
    ));

    commands.spawn(SceneRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset(asset_path("models/FlightHelmet/FlightHelmet.gltf")),
    )));
}

fn animate_light(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<DirectionalLight>>,
) {
    for mut transform in &mut query {
        transform.rotation = Quat::from_euler(
            EulerRot::ZYX,
            0.0,
            time.elapsed_secs() * PI / 5.0,
            -FRAC_PI_4,
        );
    }
}
