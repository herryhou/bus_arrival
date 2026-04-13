//! Binary route data reader using shared zero-copy implementation.
//!
//! This module provides an interface to the route data stored in Flash memory,
//! utilizing the shared `binfile` implementation for CRC32 verification and
//! zero-copy access to nodes, stops, and the spatial grid.

pub use shared::binfile::{BusError, RouteData, MAGIC, VERSION};
