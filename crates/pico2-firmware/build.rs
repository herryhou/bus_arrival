use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Generate LUTs from pipeline probability module
    // We don't specify --target, letting cargo choose the host target
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "gen_luts", "--features", "std"])
        .current_dir("../pipeline/detection")
        .output()
        .expect("Failed to run LUT generator");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("LUT generator failed: {}", stderr);
    }

    let lut_content = String::from_utf8_lossy(&output.stdout).into_owned();

    // Validate LUT values are in valid u8 range
    for line in lut_content.lines() {
        if line.trim().starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(value_str) = line.split_whitespace().last() {
                if let Ok(value) = value_str.trim_end_matches(',').parse::<i32>() {
                    if value < 0 || value > 255 {
                        panic!("LUT value out of range [0, 255]: {}", value);
                    }
                }
            }
        }
    }

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
