//! Control layer — state machine and orchestration
//!
//! This layer manages system mode (Normal/OffRoute/Recovering) and
//! orchestrates the isolated estimation and detection layers.

pub mod mode;
pub mod timeout;

pub use mode::{SystemMode, TransitionAction};
pub use timeout::{check_recovering_timeout, find_closest_stop_index};
