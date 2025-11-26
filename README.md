# dioxus-bevy

> Embed Bevy rendering in Dioxus Native applications with proper lifecycle management.

[![Crates.io](https://img.shields.io/crates/v/dioxus-bevy.svg)](https://crates.io/crates/dioxus-bevy)
[![Documentation](https://docs.rs/dioxus-bevy/badge.svg)](https://docs.rs/dioxus-bevy)
[![License](https://img.shields.io/crates/l/dioxus-bevy.svg)](https://github.com/yourusername/dioxus-bevy#license)

This crate is just a simple integration layer between [Bevy](https://bevyengine.org/)'s rendering engine and [Dioxus](https://dioxuslabs.com/), reducing boilerplate and lets you use bevy just like other dioxus components. 

## Quick Start

Add to your `Cargo.toml`:
```toml
[dependencies]
dioxus = "0.7"
dioxus-bevy = "0.1"
bevy = { version = "0.17", default-features = false, features = ["bevy_render", "bevy_core_pipeline", "bevy_winit"] }
```


### How It Works
**Dioxus owns the window**, and Bevy renders to a texture that's displayed via `CustomPaintSource`:

## A Quick View
```rust
use dioxus::prelude::*;
use dioxus_bevy::{BevyComponent};

fn main() {
    dioxus_native::launch_cfg(App, Vec::new(), dioxus_bevy::config());
}

#[component]
fn App() -> Element {
    rsx! {
        MyComponent {}
    }
}

#[bevy_component]
fn my_component(app: &mut App) {
    app.add_systems(Startup, setup);
}
```

## Examples
Run the examples:

```bash
# Simple colored triangle
cargo run --example hello_triangle

# Interactive 3D cube with signal-based props
cargo run --example interactive_cube

# GLTF model with asset management
cargo run --example gltf_model
```


## License

Licensed under MIT license

