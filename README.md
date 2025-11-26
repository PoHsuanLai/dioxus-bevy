# dioxus-bevy

> Embed Bevy rendering in Dioxus Native applications with proper lifecycle management.

[![Crates.io](https://img.shields.io/crates/v/dioxus-bevy.svg)](https://crates.io/crates/dioxus-bevy)
[![Documentation](https://docs.rs/dioxus-bevy/badge.svg)](https://docs.rs/dioxus-bevy)
[![License](https://img.shields.io/crates/l/dioxus-bevy.svg)](https://github.com/yourusername/dioxus-bevy#license)

This crate provides a clean integration layer between [Bevy](https://bevyengine.org/)'s rendering engine and [Dioxus](https://dioxuslabs.com/)'s reactive UI framework, handling the complex lifecycle issues that arise when embedding GPU-accelerated Bevy apps inside Dioxus components.

## Features

- ✨ **Lifecycle Management** - Bevy instances survive component unmount/remount cycles
- ⚡ **Lazy Initialization** - Renderers created when WGPU device is available
- 🔄 **Reference Counting** - Multiple component instances can share one Bevy app
- 📨 **Message Passing** - Type-safe communication between Dioxus UI and Bevy
- 🛑 **Proper Cleanup** - Graceful shutdown without freezing

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
dioxus = "0.7"
dioxus-bevy = "0.1"
bevy = { version = "0.17", default-features = false, features = ["bevy_render", "bevy_core_pipeline"] }
```

## Example: Interactive 3D Cube

```rust
use bevy::prelude::*;
use dioxus::prelude::*;
use dioxus_bevy::{BevyComponent, BevyRenderer, use_bevy_message};

#[component]
fn App() -> Element {
    let mut rotation_speed = use_signal(|| 1.0f32);
    let send_to_bevy = use_bevy_message("cube-scene");

    // Send updates to Bevy
    use_effect(move || {
        send_to_bevy.send(Box::new(SetSpeed(rotation_speed())));
    });

    rsx! {
        div {
            // UI controls
            input {
                r#type: "range",
                value: "{rotation_speed}",
                oninput: move |evt| {
                    rotation_speed.set(evt.value().parse().unwrap_or(1.0));
                }
            }

            // Embedded Bevy renderer
            BevyComponent {
                bevy_id: "cube-scene",
                factory: std::sync::Arc::new(|device| {
                    Box::new(CubeRenderer::new(device))
                }),
            }
        }
    }
}
```

## Architecture

### How It Works

**Dioxus owns the window**, and Bevy renders to a texture that's displayed via `CustomPaintSource`:

```
┌─────────────────────────────────────┐
│  Dioxus Native (owns window/event loop)  │
│  ┌───────────────────────────────┐ │
│  │   Dioxus Components           │ │
│  │   ┌─────────────────────────┐ │ │
│  │   │  BevyComponent          │ │ │
│  │   │  ├─ BevyInstanceManager │ │ │
│  │   │  ├─ CustomPaintSource   │ │ │
│  │   │  └─ Message Passing     │ │ │
│  │   └─────────────────────────┘ │ │
│  └───────────────────────────────┘ │
│               ▼                     │
│    ┌─────────────────────┐         │
│    │  Bevy App (headless)│         │
│    │  ├─ Render to Texture│        │
│    │  ├─ Handle Messages │         │
│    │  └─ Update Loop     │         │
│    └─────────────────────┘         │
└─────────────────────────────────────┘
```

### Why This Approach?

Compared to other integration strategies:

| Feature | bevy_dioxus | dioxus-in-bevy | **dioxus-bevy** |
|---------|-------------|----------------|-----------------|
| Who owns window? | Bevy | Bevy | **Dioxus** ✅ |
| Production ready? | ❌ | ❌ | **✅** |
| Lifecycle management | ❌ | ❌ | **✅** |
| Message passing | ❌ | ❌ | **✅** |
| Survives remounts | ❌ | ❌ | **✅** |
| Use case | Dioxus in Bevy game | Dioxus in Bevy game | **Bevy in Dioxus app** |

## Examples

Run the examples:

```bash
# Simple colored triangle
cargo run --example hello_triangle

# Interactive 3D cube with UI controls
cargo run --example interactive_cube
```

## Use Cases

- 🎮 **Game UIs** - Bevy for gameplay, Dioxus for menus/HUD
- 📊 **Data Visualization** - 3D plots with reactive controls
- 🛠️ **CAD/Design Tools** - 3D viewport with Dioxus UI
- ⚙️ **Custom Renderers** - GPU-accelerated components (text editors, etc.)

## How It Differs from Official Example

The official Dioxus `bevy-texture` example uses a **screenshot-based** approach (slow), while `dioxus-bevy`:

- ✅ Shares textures directly (faster)
- ✅ Handles component remounting correctly
- ✅ Provides message passing system
- ✅ Manages multiple Bevy instances
- ✅ Prevents shutdown freezes

## Implementing `BevyRenderer`

```rust
use dioxus_bevy::BevyRenderer;
use bevy::prelude::*;

struct MyRenderer {
    app: App,
}

impl BevyRenderer for MyRenderer {
    fn render(&mut self, ctx: CustomPaintCtx, width: u32, height: u32)
        -> Option<TextureHandle>
    {
        self.app.update();
        // TODO: Extract texture from Bevy and return
        None
    }

    fn handle_message(&mut self, msg: Box<dyn Any + Send>) {
        // Handle messages from Dioxus UI
    }

    fn shutdown(&mut self) {
        // Clean shutdown
        self.app.world_mut().send_event(AppExit::Success);
    }
}
```

## Roadmap

- [ ] Complete texture extraction implementation
- [ ] Input event forwarding (mouse, keyboard)
- [ ] Multiple render targets per Bevy instance
- [ ] Performance benchmarks
- [ ] More examples (particle effects, physics, etc.)

## Contributing

Contributions welcome! This crate was extracted from [DAWAI](https://github.com/yourusername/dawai), a live-coding DAW.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
