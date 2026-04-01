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
