// Offline preprocessor for GPS bus arrival detection system
//
// Phase 1: Route simplification, stop projection, and binary packing

use std::env;
use std::fs;
use std::process;

mod coord;
mod input;
mod linearize;
mod pack;
mod route;
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
    println!();

    // Compute average latitude for coordinate conversion
    let lat_avg: f64 = if route_input.route_points.is_empty() {
        eprintln!("Error: Route has no points");
        process::exit(1);
    } else {
        route_input.route_points.iter().map(|p| p.lat).sum::<f64>()
            / route_input.route_points.len() as f64
    };
    println!("Average latitude: {:.6}°", lat_avg);

    // Use FIXED origin at (120.0°E, 20.0°N) - same for all routes
    let grid_origin = (coord::FIXED_ORIGIN_X_CM as i32, coord::FIXED_ORIGIN_Y_CM as i32);
    println!("Grid origin: FIXED at (120.0°E, 20.0°N) = ({}, {}) cm", grid_origin.0, grid_origin.1);

    // Convert all coordinates to grid-relative coordinates (using fixed origin)
    let route_points_grid: Vec<(i64, i64)> = route_input.route_points
        .iter()
        .map(|p| {
            let (dx, dy) = coord::latlon_to_cm_relative(p.lat, p.lon, lat_avg);
            (dx as i64, dy as i64)
        })
        .collect();

    // Simplify route using Douglas-Peucker algorithm
    let simplified_indices = simplify::douglas_peucker(&route_points_grid, 100.0, &[]); // 1m threshold in cm
    let simplified_route: Vec<(i64, i64)> = simplified_indices
        .iter()
        .map(|&i| route_points_grid[i])
        .collect();

    println!("Simplified route: {} -> {} points", route_points_grid.len(), simplified_route.len());

    // Linearize route into linked nodes
    let route_nodes = linearize::build_route_graph(&simplified_route, &route_input.route_points, lat_avg);
    println!("Built route graph with {} nodes", route_nodes.len());

    // Project stops onto route
    let mut stops = stops_input.stops;
    let mut projected_stops: Vec<stops::Stop> = Vec::new();

    for stop in &mut stops {
        // Convert stop to grid-relative coordinates (using fixed origin)
        let (stop_x_rel, stop_y_rel) = coord::latlon_to_cm_relative(stop.lat, stop.lon, lat_avg);

        let stop_grid = stops::PointCM {
            x_cm: stop_x_rel as i64,
            y_cm: stop_y_rel as i64,
        };

        // Project onto route
        let (route_node_index, lat_cm, lon_cm) = stops::project_stop(&stop_grid, &route_nodes);

        projected_stops.push(stops::Stop {
            lat_cm,
            lon_cm,
            route_node_index,
        });
    }

    println!("Projected {} stops onto route", projected_stops.len());
    println!();

    // Write binary output
    println!("Writing binary data to {}", output_bin_path);
    let output_file = fs::File::create(output_bin_path)
        .unwrap_or_else(|e| {
            eprintln!("Error creating output file {}: {}", output_bin_path, e);
            process::exit(1);
        });

    if let Err(e) = pack::pack_route_data(&route_nodes, &projected_stops, grid_origin, output_file) {
        eprintln!("Error writing binary data: {}", e);
        process::exit(1);
    }

    println!("Successfully wrote {} bytes", fs::metadata(output_bin_path).unwrap().len());
    println!();
    println!("========================================");
    println!("Preprocessing complete!");
    println!("========================================");
}
