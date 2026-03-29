//! Common utilities for simulator tests

use std::path::PathBuf;

/// Get the path to the test assets directory
pub fn test_assets_path() -> PathBuf {
    // Get the path relative to this test file
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("test_assets");
    path
}

/// Get the full path for a test asset file
pub fn test_asset_file(name: &str) -> PathBuf {
    let mut path = test_assets_path();
    path.push(name);
    path
}

/// Load test asset bytes
pub fn load_test_asset_bytes(name: &str) -> Vec<u8> {
    let path = test_asset_file(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read test asset {}: {:?}. Hint: Run `cargo run -p preprocessor -- test_data/ty225_route.json test_data/ty225_stops.json {}`", name, e, path.display()))
}
