//! # dioxus-bevy
//!
//! Embed Bevy rendering in Dioxus Native applications with proper lifecycle management.
//!
//! This crate provides a simple integration layer between Bevy's rendering engine and
//! Dioxus's reactive UI framework. 
//!
//! ## Features
//!
//! - **Lifecycle Management**: Bevy instances survive component unmount/remount cycles
//! - **Lazy Initialization**: Renderers created when WGPU device is available
//! - **Reference Counting**: Multiple component instances shares one Bevy app
//! - **Message Passing**: Type-safe communication between Dioxus UI and Bevy
//! - **Proper Cleanup**: Shutdown without freezing
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dioxus::prelude::*;
//! use dioxus_bevy::{BevyComponent};
//!
//! #[component]
//! fn App() -> Element {
//!     rsx! {
//!         MyComponent {}
//!     }
//! }
//!
//! #[bevy_component]
//! fn my_component(app: &mut App) {
//!     app.add_systems(Startup, setup);
//! }
//! ```

// Re-export the macro
pub use dioxus_bevy_macro::bevy_component;

use dioxus::prelude::*;
use dioxus_core::{use_hook_with_cleanup, ScopeId};
use dioxus_native::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle, DioxusNativeWindowRenderer};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Unique identifier for a Bevy instance
///
/// Uses Dioxus's ScopeId, which is unique per component instance and
/// automatically managed by Dioxus's lifecycle system.
pub type BevyInstanceId = ScopeId;

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
    fn resume(&mut self, _device: &DeviceHandle) {}

    /// Shutdown (cleanup before destruction)
    fn shutdown(&mut self) {}
}

/// Paint source wrapper for a managed Bevy instance
///
/// Internal implementation detail that bridges Dioxus's CustomPaintSource
/// with the Bevy instance manager. Handles lazy initialization and lifecycle.
pub(crate) struct ManagedBevyPaintSource {
    instance_id: BevyInstanceId,
    manager: Arc<Mutex<BevyInstanceManagerInner>>,
    factory: Option<Box<dyn FnOnce(&DeviceHandle) -> Box<dyn BevyRenderer> + Send>>,
}

impl CustomPaintSource for ManagedBevyPaintSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        let mut mgr = self.manager.lock().unwrap();

        if let Some(instance) = mgr.instances.get_mut(&self.instance_id) {
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
        if let Some(instance) = mgr.instances.get_mut(&self.instance_id) {
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
        if let Some(instance) = mgr.instances.get_mut(&self.instance_id) {
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
///
/// Internal state for a single Bevy renderer, including reference counting
/// for handling multiple mount/unmount cycles.
pub(crate) struct BevyInstance {
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

/// Inner manager state (wrapped in Arc<Mutex<>>)
///
/// Contains the HashMap of all active Bevy instances. Kept separate from
/// BevyInstanceManager to allow for interior mutability through Arc<Mutex>.
pub(crate) struct BevyInstanceManagerInner {
    instances: HashMap<BevyInstanceId, BevyInstance>,
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
        instance_id: BevyInstanceId,
        dioxus_renderer: &DioxusNativeWindowRenderer,
        factory: F,
    ) -> u64
    where
        F: FnOnce(&DeviceHandle) -> Box<dyn BevyRenderer> + Send + 'static,
    {
        let mut inner = self.inner.lock().unwrap();

        if let Some(instance) = inner.instances.get_mut(&instance_id) {
            instance.ref_count += 1;
            return instance.paint_source_id.expect("Paint source not registered");
        }

        let paint_source = ManagedBevyPaintSource {
            instance_id,
            manager: self.inner.clone(),
            factory: Some(Box::new(factory)),
        };
        let paint_source_id = dioxus_renderer.register_custom_paint_source(Box::new(paint_source));

        let instance = BevyInstance {
            renderer: None,
            paint_source_id: Some(paint_source_id),
            ref_count: 1,
        };

        inner.instances.insert(instance_id, instance);
        paint_source_id
    }

    /// Release a reference to a Bevy instance
    ///
    /// Decrements the reference count. If it reaches zero, the instance is destroyed.
    pub fn release(&self, instance_id: &BevyInstanceId) {
        let mut inner = self.inner.lock().unwrap();

        if let Some(instance) = inner.instances.get_mut(instance_id) {
            instance.ref_count -= 1;

            // DON'T destroy the instance even at ref_count 0
            // This allows the instance to survive brief unmount/remount cycles during panel swaps
            // The instance will be reused when the component remounts
        }
    }

    /// Send a message to a Bevy instance
    ///
    /// The message is forwarded to the renderer's handle_message method.
    pub fn send_message(&self, instance_id: &BevyInstanceId, msg: Box<dyn Any + Send>) {
        let mut inner = self.inner.lock().unwrap();

        if let Some(instance) = inner.instances.get_mut(instance_id) {
            if let Some(renderer) = &mut instance.renderer {
                renderer.handle_message(msg);
            }
        }
    }

    /// Send a signal update to a Bevy instance
    pub fn send_signal(&self, instance_id: &BevyInstanceId, update: SignalUpdate) {
        self.send_message(instance_id, Box::new(update));
    }
}

impl Default for BevyInstanceManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Launch Config Helper
// ============================================================================

/// Get WGPU config for Bevy compatibility
///
/// Returns the config vector needed for `dioxus_native::launch_cfg`.
///
/// # Example
/// ```ignore
/// dioxus_native::launch_cfg(App, Vec::new(), dioxus_bevy::config());
/// ```
pub fn config() -> Vec<Box<dyn std::any::Any>> {
    use dioxus_native::Limits;

    let limits = Limits {
        max_storage_buffers_per_shader_stage: 12,
        ..Limits::default()
    };

    vec![Box::new(limits)]
}

// ============================================================================
// Dioxus Component API
// ============================================================================

/// Props for BevyComponent
#[derive(Props, Clone)]
pub struct BevyComponentProps {
    /// Unique ID for this Bevy instance (uses Dioxus ScopeId)
    pub instance_id: BevyInstanceId,

    /// Factory function to create the renderer (wrapped in Arc to allow Clone)
    pub factory: Arc<dyn Fn(&DeviceHandle) -> Box<dyn BevyRenderer> + Send + Sync>,

    /// Optional children (rendered as overlay on the canvas)
    #[props(default)]
    pub children: Element,
}

impl PartialEq for BevyComponentProps {
    fn eq(&self, other: &Self) -> bool {
        // Compare only instance_id, not the factory function
        self.instance_id == other.instance_id
    }
}

/// Bevy-backed component with Dioxus-like API
///
/// # Example
///
/// ```rust,ignore
/// let instance_id = current_scope_id();
/// rsx! {
///     BevyComponent {
///         instance_id,
///         factory: |device| Box::new(CodeEditorRenderer::new(device)),
///     }
/// }
/// ```
#[component]
pub fn BevyComponent(props: BevyComponentProps) -> Element {
    let manager = match try_use_context::<Signal<BevyInstanceManager>>() {
        Some(mgr) => mgr,
        None => use_context_provider(|| Signal::new(BevyInstanceManager::new())),
    };

    let renderer = use_context::<DioxusNativeWindowRenderer>();

    let paint_source_id = use_hook_with_cleanup(
        {
            let instance_id = props.instance_id;
            let factory = props.factory.clone();
            let mut mgr = manager;
            move || {
                let id = mgr.write().get_or_create(
                    instance_id,
                    &renderer,
                    move |dev| factory(dev),
                );
                (instance_id, mgr, id)
            }
        },
        move |(instance_id, mut mgr, _id)| {
            mgr.write().release(&instance_id);
        },
    ).2;

    rsx! {
        canvas {
            "src": paint_source_id,
            style: "display: block; width: 100%; height: 100%;",
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
pub fn use_bevy_message(instance_id: BevyInstanceId) -> BevyMessageSender {
    let manager = match try_use_context::<Signal<BevyInstanceManager>>() {
        Some(mgr) => mgr,
        None => use_context_provider(|| Signal::new(BevyInstanceManager::new())),
    };

    BevyMessageSender {
        instance_id,
        manager,
    }
}

/// Helper for sending messages to a Bevy component
///
/// Created by `use_bevy_message` hook. Provides methods to send arbitrary
/// messages or typed signal updates to a Bevy renderer.
#[derive(Clone)]
pub struct BevyMessageSender {
    instance_id: BevyInstanceId,
    manager: Signal<BevyInstanceManager>,
}

impl BevyMessageSender {
    /// Send a message to the Bevy component
    ///
    /// The message will be forwarded to the renderer's `handle_message` method.
    /// Use `send_signal_update` for typed signal updates.
    pub fn send(&self, msg: Box<dyn Any + Send>) {
        self.manager.peek().send_message(&self.instance_id, msg);
    }

    /// Send a typed signal update to the Bevy component
    ///
    /// Converts the value to a `SignalUpdate` and sends it via the message channel.
    /// The Bevy renderer can receive these via `SignalReceiver` resource.
    pub fn send_signal_update<T: IntoSignalUpdate>(&self, key: &str, value: T) {
        let update = value.into_signal_update(key.to_string());
        self.manager.peek().send_signal(&self.instance_id, update);
    }
}

// ============================================================================
// Bevy App Builder
// ============================================================================

use bevy::app::App;
use bevy::prelude::*;
use crossbeam_channel::{Sender, Receiver, unbounded};

/// Message sent from Dioxus signals to Bevy
///
/// Represents a typed value update from a Dioxus signal. The first String
/// is the key/name of the signal, and the second value is the new value.
/// Receive these in Bevy via the `SignalReceiver` resource.
#[derive(Debug, Clone)]
pub enum SignalUpdate {
    /// Boolean signal update: (key, value)
    Bool(String, bool),
    /// 32-bit float signal update: (key, value)
    F32(String, f32),
    /// 64-bit float signal update: (key, value)
    F64(String, f64),
    /// 32-bit integer signal update: (key, value)
    I32(String, i32),
    /// 32-bit unsigned integer signal update: (key, value)
    U32(String, u32),
    /// String signal update: (key, value)
    String(String, String),
}

/// Resource that receives signal updates from Dioxus via a channel
///
/// Add this to your Bevy app to receive typed signal updates from Dioxus.
/// Use `receiver.try_recv()` in your systems to poll for new values.
///
/// # Example
/// ```rust,ignore
/// fn my_system(receiver: Res<SignalReceiver>) {
///     while let Ok(update) = receiver.receiver.try_recv() {
///         match update {
///             SignalUpdate::F32(key, value) if key == "speed" => {
///                 // Handle speed update
///             }
///             _ => {}
///         }
///     }
/// }
/// ```
#[derive(Resource)]
pub struct SignalReceiver {
    /// Channel receiver for signal updates
    pub receiver: Receiver<SignalUpdate>,
}

/// Helper to send signal updates (stored with the component)
#[derive(Clone)]
pub struct SignalSender {
    pub sender: Sender<SignalUpdate>,
}

/// Trait for types that can be converted to SignalUpdate
///
/// Implemented for primitive types (bool, f32, f64, i32, u32, String).
/// Allows generic signal update sending via `send_signal_update`.
pub trait IntoSignalUpdate {
    /// Convert this value into a SignalUpdate with the given key
    fn into_signal_update(self, key: String) -> SignalUpdate;
}

impl IntoSignalUpdate for bool {
    fn into_signal_update(self, key: String) -> SignalUpdate {
        SignalUpdate::Bool(key, self)
    }
}

impl IntoSignalUpdate for f32 {
    fn into_signal_update(self, key: String) -> SignalUpdate {
        SignalUpdate::F32(key, self)
    }
}

impl IntoSignalUpdate for f64 {
    fn into_signal_update(self, key: String) -> SignalUpdate {
        SignalUpdate::F64(key, self)
    }
}

impl IntoSignalUpdate for i32 {
    fn into_signal_update(self, key: String) -> SignalUpdate {
        SignalUpdate::I32(key, self)
    }
}

impl IntoSignalUpdate for u32 {
    fn into_signal_update(self, key: String) -> SignalUpdate {
        SignalUpdate::U32(key, self)
    }
}

impl IntoSignalUpdate for String {
    fn into_signal_update(self, key: String) -> SignalUpdate {
        SignalUpdate::String(key, self)
    }
}

/// Helper function to create a SignalUpdate from any supported type
pub fn make_signal_update<T: IntoSignalUpdate>(key: String, value: T) -> SignalUpdate {
    value.into_signal_update(key)
}

// ============================================================================
// Asset Resolution - Integrate Bevy with Dioxus Asset System
// ============================================================================

/// Resolve an asset path to work with Dioxus's asset management.
///
/// This helper ensures that Bevy loads assets from the same `assets/` directory
/// that Dioxus uses, providing a unified asset management experience.
///
/// The function simply ensures paths are properly formatted with the `assets/` prefix,
/// which is the standard convention for both Dioxus and Bevy.
///
/// # Example
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use dioxus_bevy::asset_path;
///
/// fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
///     // Load from assets/models/cube.gltf
///     commands.spawn(SceneRoot(asset_server.load(
///         asset_path("models/cube.gltf")
///     )));
/// }
/// ```
pub fn asset_path(path: &str) -> String {
    // Normalize the path - remove leading slashes and assets/ prefix if present
    let trimmed = path.trim_start_matches('/').trim_start_matches("assets/");

    // Just return the path without adding assets/ prefix again
    // Bevy's AssetServer will handle the assets/ directory automatically
    trimmed.to_string()
}

// ============================================================================
// Helper Macros for Signal Handling
// ============================================================================

/// Convenient macro for extracting signal updates in Bevy systems.
///
/// This makes signal handling pattern similar to regular Dioxus reactive code.
///
/// # Example
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use dioxus_bevy::{SignalReceiver, extract_signals};
///
/// #[derive(Resource)]
/// struct RotationSpeed(f32);
///
/// #[derive(Resource)]
/// struct CubeColor(i32);
///
/// fn process_signals(
///     receiver: Res<SignalReceiver>,
///     mut speed: ResMut<RotationSpeed>,
///     mut color: ResMut<CubeColor>,
/// ) {
///     extract_signals!(receiver, {
///         "rotation_speed": f32 => |val| speed.0 = val,
///         "color_index": i32 => |val| color.0 = val,
///     });
/// }
/// ```
#[macro_export]
macro_rules! extract_signals {
    ($receiver:expr, { $($key:literal : f32 => |$val:ident| $action:expr),* $(,)? }) => {
        while let Ok(update) = $receiver.receiver.try_recv() {
            match update {
                $(
                    $crate::SignalUpdate::F32(key, $val) if key == $key => { $action }
                )*
                _ => {}
            }
        }
    };

    ($receiver:expr, { $($key:literal : i32 => |$val:ident| $action:expr),* $(,)? }) => {
        while let Ok(update) = $receiver.receiver.try_recv() {
            match update {
                $(
                    $crate::SignalUpdate::I32(key, $val) if key == $key => { $action }
                )*
                _ => {}
            }
        }
    };

    ($receiver:expr, { $($key:literal : u32 => |$val:ident| $action:expr),* $(,)? }) => {
        while let Ok(update) = $receiver.receiver.try_recv() {
            match update {
                $(
                    $crate::SignalUpdate::U32(key, $val) if key == $key => { $action }
                )*
                _ => {}
            }
        }
    };

    ($receiver:expr, { $($key:literal : bool => |$val:ident| $action:expr),* $(,)? }) => {
        while let Ok(update) = $receiver.receiver.try_recv() {
            match update {
                $(
                    $crate::SignalUpdate::Bool(key, $val) if key == $key => { $action }
                )*
                _ => {}
            }
        }
    };

    ($receiver:expr, { $($key:literal : String => |$val:ident| $action:expr),* $(,)? }) => {
        while let Ok(update) = $receiver.receiver.try_recv() {
            match update {
                $(
                    $crate::SignalUpdate::String(key, ref val_ref) if key == $key => {
                        let $val = val_ref.clone();
                        $action
                    }
                )*
                _ => {}
            }
        }
    };
}

/// Helper trait for creating Bevy resources from signal values
pub trait FromSignalUpdate: Sized {
    /// Create a resource from a signal update value
    fn from_signal<T: IntoSignalUpdate>(value: T) -> Self
    where
        Self: From<T>;
}

/// Convenient wrapper for creating a Bevy renderer
///
/// Provides a high-level API for embedding Bevy apps in Dioxus components.
/// Handles texture management, WGPU device sharing, and signal passing.
///
/// # Example
/// ```rust,ignore
/// BevyAppRenderer::new(device, |app| {
///     app.add_systems(Startup, setup_scene);
///     app.add_systems(Update, rotate_cube);
/// })
/// ```
pub struct BevyAppRenderer {
    app: App,
    wgpu_device: wgpu::Device,
    texture_handle: Option<TextureHandle>,
    manual_texture_view_handle: Option<bevy::camera::ManualTextureViewHandle>,
    last_texture_size: (u32, u32),
    pub signal_sender: SignalSender,
}

// SAFETY: Bevy App is only accessed from main thread via Mutex in BevyInstanceManager
unsafe impl Send for BevyAppRenderer {}

impl BevyAppRenderer {
    /// Create a new Bevy renderer with a setup function
    ///
    /// # Example
    /// ```ignore
    /// BevyAppRenderer::new(device, |app| {
    ///     app.add_systems(Startup, setup_scene);
    /// })
    /// ```
    pub fn new<F>(device: &DeviceHandle, setup: F) -> Self
    where
        F: FnOnce(&mut App)
    {
        use bevy::render::{
            renderer::{RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue, WgpuWrapper},
            settings::{RenderCreation, RenderResources},
            texture::ManualTextureViews,
            RenderPlugin,
        };
        use std::sync::Arc;

        let mut app = App::new();

        // Add Bevy plugins (headless mode) - SHARE WGPU RESOURCES WITH DIOXUS
        app.add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: RenderCreation::Manual(RenderResources(
                        RenderDevice::new(WgpuWrapper::new(device.device.clone())),
                        RenderQueue(Arc::new(WgpuWrapper::new(device.queue.clone()))),
                        RenderAdapterInfo(WgpuWrapper::new(device.adapter.get_info())),
                        RenderAdapter(Arc::new(WgpuWrapper::new(device.adapter.clone()))),
                        RenderInstance(Arc::new(WgpuWrapper::new(device.instance.clone()))),
                    )),
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>(),
        );

        // Clear color (transparent by default)
        app.insert_resource(ClearColor(Color::srgba(0.0, 0.0, 0.0, 0.0)));

        // Add manual texture views resource
        app.insert_resource(ManualTextureViews::default());

        // Create channel for signal updates
        let (sender, receiver) = unbounded();
        app.insert_resource(SignalReceiver { receiver });

        // User setup
        setup(&mut app);

        // Initialize
        app.finish();
        app.cleanup();
        app.update();

        Self {
            app,
            wgpu_device: device.device.clone(),
            texture_handle: None,
            manual_texture_view_handle: None,
            last_texture_size: (0, 0),
            signal_sender: SignalSender { sender },
        }
    }

    fn init_texture(&mut self, mut ctx: CustomPaintCtx<'_>, width: u32, height: u32) {
        use bevy::camera::{Camera, ManualTextureViewHandle, RenderTarget};
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
        use bevy::render::texture::{ManualTextureView, ManualTextureViews};

        if width == 0 || height == 0 {
            return;
        }

        let current_size = (width, height);
        if self.texture_handle.is_some() && self.last_texture_size == current_size {
            return;
        }

        let world = self.app.world_mut();

        let mut camera_query = world.query::<&Camera>();
        let camera_count = camera_query.iter(world).count();
        if camera_count == 0 {
            return;
        }

        if let Some(mut manual_texture_views) = world.get_resource_mut::<ManualTextureViews>() {
            if self.texture_handle.is_some() {
                ctx.unregister_texture(self.texture_handle.take().unwrap());
            }
            if let Some(old_handle) = self.manual_texture_view_handle {
                manual_texture_views.remove(&old_handle);
                self.manual_texture_view_handle = None;
            }

            let format = TextureFormat::Rgba8UnormSrgb;
            let wgpu_texture = self.wgpu_device.create_texture(&wgpu::TextureDescriptor {
                label: Some("bevy_texture"),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let wgpu_texture_view =
                wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let manual_texture_view = ManualTextureView {
                texture_view: wgpu_texture_view.into(),
                size: bevy::math::UVec2::new(width, height),
                format,
            };
            let manual_texture_view_handle = ManualTextureViewHandle(0);
            manual_texture_views.insert(manual_texture_view_handle, manual_texture_view);

            if let Ok(mut camera) = world.query::<&mut Camera>().single_mut(world) {
                camera.target = RenderTarget::TextureView(manual_texture_view_handle);

                self.last_texture_size = current_size;
                self.manual_texture_view_handle = Some(manual_texture_view_handle);
                self.texture_handle = Some(ctx.register_texture(wgpu_texture));
            }
        }
    }
}

impl BevyRenderer for BevyAppRenderer {
    fn render(&mut self, ctx: CustomPaintCtx, width: u32, height: u32) -> Option<TextureHandle> {
        self.init_texture(ctx, width, height);
        self.app.update();
        self.texture_handle.clone()
    }

    fn handle_message(&mut self, msg: Box<dyn Any + Send>) {
        // Try to downcast to SignalUpdate and forward to channel
        if let Some(update) = msg.downcast_ref::<SignalUpdate>() {
            let _ = self.signal_sender.sender.send(update.clone());
        }
    }

    fn shutdown(&mut self) {
        self.app.world_mut().write_message(bevy::app::AppExit::Success);
        self.app.update();
    }
}
