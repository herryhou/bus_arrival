//! Scenario-based integration tests for the bus arrival detection pipeline
//!
//! These tests validate the complete pipeline using real ty225 route data
//! across various scenarios: normal operation, GPS anomalies, signal loss,
//! and route geometry edge cases.

mod common;
mod normal;
mod gps_anomalies;
mod signal_loss;
mod route_edge_cases;
mod edge_cases;
mod detour_reentry_integration;
mod normal_trace_validation;
mod snap_recovery_coordination;
