//! Stop projection and K-candidate selection

pub mod generator;

pub use generator::{generate_candidates, generate_candidates_with_snap};

/// Candidate projection for a stop
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Segment index [0, N-2]
    pub seg_idx: usize,
    /// Position along segment [0.0, 1.0]
    pub t: f64,
    /// Squared distance to stop (cm²)
    pub dist_sq_cm2: i64,
    /// Cumulative progress from route start (cm)
    pub progress_cm: i32,
}
