//! Arrival detection for bus arrival detection system.
//! Supports no_std embedded targets.

#![cfg_attr(not(feature = "std"), no_std)]


pub mod corridor;
pub mod probability;
pub mod state_machine;
pub mod recovery;

#[cfg(feature = "std")]
pub mod input;

#[cfg(feature = "std")]
pub mod output;

#[cfg(feature = "std")]
pub mod trace;

// Re-export commonly used types
pub use state_machine::{StopState, StopEvent};
pub use probability::{compute_probability_with_luts, THETA_ARRIVAL};

#[cfg(feature = "std")]
pub use probability::compute_probability;
