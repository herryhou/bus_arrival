//! Pico 2 firmware library for bus arrival detection
//!
//! This library exposes the core firmware functionality for testing on host.

#![cfg_attr(not(feature = "dev"), no_std)]

pub mod control;
pub mod detection;
pub mod estimation;
pub mod lut;
pub mod recovery;
pub mod recovery_trigger;
pub mod state;

// Re-export key types for external use
pub use control::{SystemMode, SystemState};

// uart module depends on Embassy and is only available for firmware builds
#[cfg(feature = "firmware")]
pub mod uart;
