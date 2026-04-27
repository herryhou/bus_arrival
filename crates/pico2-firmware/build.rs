use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn run_with_timeout(cmd: &mut Command, timeout_secs: u64) -> std::io::Result<std::process::Output> {
    let timeout = Duration::from_secs(timeout_secs);
    let mut child = cmd.spawn()?;

    let start = std::time::Instant::now();
    loop {
        if let Some(_status) = child.try_wait()? {
            return child.wait_with_output();
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Command timed out after {} seconds", timeout_secs),
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    // Use a SEPARATE target directory to avoid circular build locks
    let target_dir = out_dir.join("lut-gen-target");

    // Detect the actual host architecture using uname
    let actual_host = {
        let output = std::process::Command::new("uname")
            .args(["-m"])
            .output()
            .expect("Failed to run uname");
        let machine = String::from_utf8_lossy(&output.stdout).trim().to_string();
        match machine.as_str() {
            "x86_64" => "x86_64-apple-darwin",
            "arm64" => "aarch64-apple-darwin",
            _ => panic!("Unknown machine architecture: {}", machine),
        }
    };

    // First, build the gen_luts binary for the ACTUAL host architecture
    let build_output = run_with_timeout(
        Command::new(&cargo)
            .args([
                "build",
                "--package",
                "detection",
                "--bin",
                "gen_luts",
                "--features",
                "std",
                "--target",
                actual_host,
            ])
            .current_dir("..")
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("CARGO_BUILD_INCREMENTAL", "false")
            .env("CARGO_BUILD_TARGET", actual_host),
        120, // 2 minutes for building
    )
    .expect("LUT generator build timed out or failed");

    if !build_output.status.success() {
        let stderr = String::from_utf8_lossy(&build_output.stderr);
        panic!("Failed to build gen_luts binary: {}", stderr);
    }

    // Then run the binary directly and capture its output
    let gen_luts_path = target_dir.join(actual_host).join("debug").join("gen_luts");

    let output = Command::new(&gen_luts_path)
        .output()
        .expect("Failed to execute gen_luts");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("gen_luts binary failed: {}", stderr);
    }

    let lut_content = String::from_utf8_lossy(&output.stdout).into_owned();

    if lut_content.is_empty() {
        panic!(
            "LUT generator produced no output! stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Validate LUT values are in valid u8 range
    for line in lut_content.lines() {
        if line.trim().starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(value_str) = line.split_whitespace().last() {
                if let Ok(value) = value_str.trim_end_matches(',').parse::<i32>() {
                    if !(0..=255).contains(&value) {
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
    let route_data_path = PathBuf::from("test_data/ty225_normal.bin");
    if route_data_path.exists() {
        println!("cargo:rerun-if-changed={}", route_data_path.display());
    }
}
