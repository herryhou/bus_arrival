//! Stop projection and K-candidate selection

pub mod generator;

pub use generator::{generate_candidates, generate_candidates_with_snap};

/// Candidate projection for a stop
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub seg_idx: usize,
    pub t: f64,
    pub dist_sq_cm2: i64,
    pub progress_cm: i32,
}
