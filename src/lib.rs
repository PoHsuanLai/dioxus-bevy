//! # dioxus-bevy
//!
//! Embed Bevy rendering in Dioxus Native applications with proper lifecycle management.
//!
//! This crate provides a clean integration layer between Bevy's rendering engine and
//! Dioxus's reactive UI framework, handling the complex lifecycle issues that arise when
//! embedding GPU-accelerated Bevy apps inside Dioxus components.
//!
//! ## Features
//!
//! - **Lifecycle Management**: Bevy instances survive component unmount/remount cycles
//! - **Lazy Initialization**: Renderers created when WGPU device is available
//! - **Reference Counting**: Multiple component instances can share one Bevy app
//! - **Message Passing**: Type-safe communication between Dioxus UI and Bevy
//! - **Proper Cleanup**: Graceful shutdown without freezing
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dioxus::prelude::*;
//! use dioxus_bevy::{BevyComponent, BevyRenderer, use_bevy_message};
//!
//! #[component]
//! fn App() -> Element {
//!     rsx! {
//!         BevyComponent {
//!             bevy_id: "my-renderer",
//!             factory: |device| Box::new(MyBevyRenderer::new(device)),
//!         }
//!     }
//! }
//! ```

use dioxus::prelude::*;
use dioxus_core::{use_hook_with_cleanup, consume_context};
use dioxus_native::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle, DioxusNativeWindowRenderer};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trait for Bevy-backed renderers
///
/// Implement this to create a component that uses Bevy for rendering.
/// Note: Only `Send` is required, not `Sync`, since renderers are accessed via `&mut self`.
pub trait BevyRenderer: Send {
    /// Render to texture
    fn render(&mut self, ctx: CustomPaintCtx, width: u32, height: u32) -> Option<TextureHandle>;

    /// Handle messages (input events, state changes, etc.)
    fn handle_message(&mut self, msg: Box<dyn Any + Send>);

    /// Suspend (optional cleanup when hidden)
    fn suspend(&mut self) {}

    /// Resume (reinitialize when shown)
    fn resume(&mut self, device: &DeviceHandle) {}

    /// Shutdown (cleanup before destruction)
    fn shutdown(&mut self) {}
}

/// Paint source wrapper for a managed Bevy instance
struct ManagedBevyPaintSource {
    bevy_id: String,
    manager: Arc<Mutex<BevyInstanceManagerInner>>,
    factory: Option<Box<dyn FnOnce(&DeviceHandle) -> Box<dyn BevyRenderer> + Send>>,
}

impl CustomPaintSource for ManagedBevyPaintSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        let mut mgr = self.manager.lock().unwrap();

        // Lazy initialization: create renderer if it doesn't exist yet
        if let Some(instance) = mgr.instances.get_mut(&self.bevy_id) {
            if instance.renderer.is_none() {
                if let Some(factory) = self.factory.take() {
                    instance.renderer = Some(factory(device_handle));
                }
            }

            if let Some(renderer) = &mut instance.renderer {
                renderer.resume(device_handle);
            }
        }
    }

    fn suspend(&mut self) {
        let mut mgr = self.manager.lock().unwrap();
        if let Some(instance) = mgr.instances.get_mut(&self.bevy_id) {
            if let Some(renderer) = &mut instance.renderer {
                renderer.suspend();
            }
        }
    }

    fn render(
        &mut self,
        ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        _scale: f64,
    ) -> Option<TextureHandle> {
        let mut mgr = self.manager.lock().unwrap();
        if let Some(instance) = mgr.instances.get_mut(&self.bevy_id) {
            if let Some(renderer) = &mut instance.renderer {
                renderer.render(ctx, width, height)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Managed Bevy instance
struct BevyInstance {
    renderer: Option<Box<dyn BevyRenderer>>,
    paint_source_id: Option<u64>,
    ref_count: usize,
}

impl Drop for BevyInstance {
    fn drop(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.shutdown();
        }
    }
}

/// Inner manager (wrapped in Arc<Mutex<>>)
struct BevyInstanceManagerInner {
    instances: HashMap<String, BevyInstance>,
}

/// Global Bevy instance manager
///
/// Manages lifecycle of all Bevy-backed components in the application.
/// Ensures only one Bevy app per component type exists, using reference counting
/// to handle multiple mount/unmount cycles.
#[derive(Clone)]
pub struct BevyInstanceManager {
    inner: Arc<Mutex<BevyInstanceManagerInner>>,
}

impl BevyInstanceManager {
    /// Create a new Bevy instance manager
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BevyInstanceManagerInner {
                instances: HashMap::new(),
            })),
        }
    }

    /// Get or create a Bevy instance
    ///
    /// Returns the paint source ID that can be used with a canvas element.
    /// If the instance already exists, increments the reference count.
    /// If not, creates a new instance slot and registers paint source.
    /// The actual renderer is created lazily in resume() when device is available.
    pub fn get_or_create<F>(
        &self,
        bevy_id: &str,
        dioxus_renderer: &DioxusNativeWindowRenderer,
        factory: F,
    ) -> u64
    where
        F: FnOnce(&DeviceHandle) -> Box<dyn BevyRenderer> + Send + 'static,
    {
        let mut inner = self.inner.lock().unwrap();

        if let Some(instance) = inner.instances.get_mut(bevy_id) {
            // Instance exists, increment ref count
            instance.ref_count += 1;
            return instance.paint_source_id.expect("Paint source not registered");
        }

        // Create new instance slot (without renderer yet)

        // Register paint source with Dioxus
        let paint_source = ManagedBevyPaintSource {
            bevy_id: bevy_id.to_string(),
            manager: self.inner.clone(),
            factory: Some(Box::new(factory)),
        };
        let paint_source_id = dioxus_renderer.register_custom_paint_source(Box::new(paint_source));

        let instance = BevyInstance {
            renderer: None, // Lazy initialization
            paint_source_id: Some(paint_source_id),
            ref_count: 1,
        };

        inner.instances.insert(bevy_id.to_string(), instance);
        paint_source_id
    }

    /// Release a reference to a Bevy instance
    ///
    /// Decrements the reference count. If it reaches zero, the instance is destroyed.
    pub fn release(&self, bevy_id: &str) {
        let mut inner = self.inner.lock().unwrap();

        if let Some(instance) = inner.instances.get_mut(bevy_id) {
            instance.ref_count -= 1;

            // DON'T destroy the instance even at ref_count 0
            // This allows the instance to survive brief unmount/remount cycles during panel swaps
            // The instance will be reused when the component remounts
        }
    }

    /// Send a message to a Bevy instance
    ///
    /// The message is forwarded to the renderer's handle_message method.
    pub fn send_message(&self, bevy_id: &str, msg: Box<dyn Any + Send>) {
        let mut inner = self.inner.lock().unwrap();

        if let Some(instance) = inner.instances.get_mut(bevy_id) {
            if let Some(renderer) = &mut instance.renderer {
                renderer.handle_message(msg);
            } else {
                tracing::warn!("⚠️  Attempted to send message to Bevy instance '{}' before renderer was initialized", bevy_id);
            }
        } else {
            tracing::warn!("⚠️  Attempted to send message to non-existent Bevy instance '{}'", bevy_id);
        }
    }
}

impl Default for BevyInstanceManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Dioxus Component API
// ============================================================================

/// Props for BevyComponent
#[derive(Props, Clone)]
pub struct BevyComponentProps {
    /// Unique ID for this Bevy instance (e.g., "code-editor")
    pub bevy_id: String,

    /// Factory function to create the renderer (wrapped in Arc to allow Clone)
    pub factory: Arc<dyn Fn(&DeviceHandle) -> Box<dyn BevyRenderer> + Send + Sync>,

    /// Optional children (rendered as overlay on the canvas)
    #[props(default)]
    pub children: Element,
}

impl PartialEq for BevyComponentProps {
    fn eq(&self, other: &Self) -> bool {
        // Compare only bevy_id, not the factory function
        self.bevy_id == other.bevy_id
    }
}

/// Bevy-backed component with Dioxus-like API
///
/// # Example
///
/// ```rust,ignore
/// rsx! {
///     BevyComponent {
///         bevy_id: "code-editor",
///         factory: |device| Box::new(CodeEditorRenderer::new(device)),
///     }
/// }
/// ```
#[component]
pub fn BevyComponent(props: BevyComponentProps) -> Element {
    let manager = use_context::<Signal<BevyInstanceManager>>();
    let renderer = use_context::<DioxusNativeWindowRenderer>();

    // On mount: get or create instance
    // On unmount: release instance
    let paint_source_id = use_hook_with_cleanup(
        {
            let bevy_id = props.bevy_id.clone();
            let factory = props.factory.clone();
            let mut mgr = manager;
            move || {
                let id = mgr.write().get_or_create(
                    &bevy_id,
                    &renderer,
                    move |dev| factory(dev),
                );
                (bevy_id, mgr, id)
            }
        },
        move |(bevy_id, mut mgr, _id)| {
            mgr.write().release(&bevy_id);
        },
    ).2;

    rsx! {
        div {
            style: "width: 100%; height: 100%; position: relative;",

            // Canvas for Bevy rendering
            canvas {
                "src": paint_source_id,
                style: "display: block; width: 100%; height: 100%;",
            }

            // Optional children overlay
            {props.children}
        }
    }
}

/// Hook to send messages to a Bevy component
///
/// # Example
///
/// ```rust,ignore
/// let send_to_editor = use_bevy_message("code-editor");
///
/// onclick: move |_| {
///     send_to_editor.send(Box::new(CodeEditorMessage::SetText("hello".to_string())));
/// }
/// ```
pub fn use_bevy_message(bevy_id: &str) -> BevyMessageSender {
    let manager = use_context::<Signal<BevyInstanceManager>>();
    let bevy_id = bevy_id.to_string();

    BevyMessageSender {
        bevy_id,
        manager,
    }
}

/// Helper for sending messages to a Bevy component
#[derive(Clone)]
pub struct BevyMessageSender {
    bevy_id: String,
    manager: Signal<BevyInstanceManager>,
}

impl BevyMessageSender {
    /// Send a message to the Bevy component
    pub fn send(&self, msg: Box<dyn Any + Send>) {
        self.manager.read().send_message(&self.bevy_id, msg);
    }
}
