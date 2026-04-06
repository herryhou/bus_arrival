use std::path::PathBuf;

fn main() {
    // Track the route data file dependency for rebuild triggers
    // The data is embedded via include_bytes! in main.rs
    let route_data_path = PathBuf::from("test_data/ty225_normal.bin");
    if route_data_path.exists() {
        println!("cargo:rerun-if-changed={}", route_data_path.display());
    }
}
