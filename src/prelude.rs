//! Prelude module for convenient imports.
//!
//! This module re-exports the most commonly used items from the crate.
//!
//! # Example
//!
//! ```rust
//! use dioxus_bevy::prelude::*;
//! ```

// Main component
pub use crate::BevyComponent;

// Procedural macro
pub use crate::bevy_component;

// Core renderer trait
pub use crate::BevyRenderer;

// Message passing system
pub use crate::{
    use_bevy_message,
    BevyMessageSender,
    SignalUpdate,
    SignalReceiver,
};

// Helper functions
pub use crate::{
    config,
    asset_path,
};

// Convenience type alias
pub use crate::BevyInstanceId;
