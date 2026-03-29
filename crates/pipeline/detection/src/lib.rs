pub mod input;
pub mod corridor;
pub mod probability;
pub mod state_machine;
pub mod recovery;
pub mod output;
pub mod trace;

// Re-export commonly used types
pub use state_machine::{StopState, StopEvent};
pub use probability::{compute_probability, THETA_ARRIVAL};
