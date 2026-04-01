use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let route_data_path = out_dir.join("route_data.bin");

    // Copy route_data.bin from test_data if it exists
    let source_path = PathBuf::from("test_data/route_data.bin");
    if source_path.exists() {
        fs::copy(&source_path, &route_data_path).unwrap();
        println!("cargo:rerun-if-changed={}", source_path.display());
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
}
