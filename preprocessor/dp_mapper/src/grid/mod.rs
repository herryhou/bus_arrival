//! Spatial indexing for O(k) segment queries

pub mod builder;

pub use builder::{build_grid, query_neighbors};
