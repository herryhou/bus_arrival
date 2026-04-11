use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Generate LUTs from pipeline probability module
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "gen_luts", "--features", "std", "--target", x86_64_apple_darwin()])
        .current_dir("../pipeline/detection")
        .output()
        .expect("Failed to run LUT generator");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("LUT generator failed: {}", stderr);
    }

    let lut_content = String::from_utf8(output.stdout).unwrap();

    let out_path = out_dir.join("lut_generated.rs");
    std::fs::write(&out_path, lut_content).expect("Failed to write LUT file");

    println!("cargo:rerun-if-changed=../shared/src/probability_constants.rs");
    println!("cargo:rerun-if-changed=../pipeline/detection/src/probability.rs");

    // Track the route data file dependency for rebuild triggers
    // The data is embedded via include_bytes! in main.rs
    let route_data_path = PathBuf::from("test_data/ty225_normal.bin");
    if route_data_path.exists() {
        println!("cargo:rerun-if-changed={}", route_data_path.display());
    }
}

#[cfg(target_os = "macos")]
fn x86_64_apple_darwin() -> &'static str {
    "x86_64-apple-darwin"
}

#[cfg(target_os = "linux")]
fn x86_64_apple_darwin() -> &'static str {
    "x86_64-unknown-linux-gnu"
}

#[cfg(target_os = "windows")]
fn x86_64_apple_darwin() -> &'static str {
    "x86_64-pc-windows-msvc"
}
