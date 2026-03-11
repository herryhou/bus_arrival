// Offline preprocessor for GPS bus arrival detection system
//
// Phase 1: Route simplification, stop projection, and binary packing

use std::env;
use std::fs;
use std::process;

mod coord;
mod input;
mod linearize;
mod simplify;
mod stops;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Require exactly 3 arguments: route.json, stops.json, route_data.bin
    if args.len() != 4 {
        eprintln!("Usage: preprocessor <route.json> <stops.json> <route_data.bin>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  route.json     - Input file containing route GPS coordinates");
        eprintln!("  stops.json     - Input file containing bus stop information");
        eprintln!("  route_data.bin - Output file for processed binary data");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  preprocessor route.json stops.json route_data.bin");
        process::exit(1);
    }

    let route_json_path = &args[1];
    let stops_json_path = &args[2];
    let output_bin_path = &args[3];

    // Print Phase 1 header
    println!("========================================");
    println!("Bus Arrival Preprocessor - Phase 1");
    println!("========================================");
    println!();
    println!("Input files:");
    println!("  Route JSON: {}", route_json_path);
    println!("  Stops JSON: {}", stops_json_path);
    println!();
    println!("Output file:");
    println!("  Binary data: {}", output_bin_path);
    println!();

    // Parse route.json
    let route_json = fs::read_to_string(route_json_path)
        .unwrap_or_else(|e| {
            eprintln!("Error reading route file {}: {}", route_json_path, e);
            process::exit(1);
        });

    let route_input: input::RouteInput = serde_json::from_str(&route_json)
        .unwrap_or_else(|e| {
            eprintln!("Error parsing route JSON: {}", e);
            process::exit(1);
        });

    println!("Parsed route with {} points", route_input.route_points.len());

    // Parse stops.json
    let stops_json = fs::read_to_string(stops_json_path)
        .unwrap_or_else(|e| {
            eprintln!("Error reading stops file {}: {}", stops_json_path, e);
            process::exit(1);
        });

    let stops_input: input::StopsInput = serde_json::from_str(&stops_json)
        .unwrap_or_else(|e| {
            eprintln!("Error parsing stops JSON: {}", e);
            process::exit(1);
        });

    println!("Parsed {} stops", stops_input.stops.len());
}
