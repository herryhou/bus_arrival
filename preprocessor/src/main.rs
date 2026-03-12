// Offline preprocessor for GPS bus arrival detection system
//
// Phase 1: Route simplification, stop projection, and binary packing (v8)

use std::env;
use std::fs;
use std::process;

mod coord;
mod grid;
mod input;
mod linearize;
mod lut;
mod pack;
mod simplify;
mod stops;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: preprocessor <route.json> <stops.json> <route_data.bin>");
        process::exit(1);
    }

    let route_json_path = &args[1];
    let stops_json_path = &args[2];
    let output_bin_path = &args[3];

    println!("========================================");
    println!("Bus Arrival Preprocessor - v8.0 Pipeline");
    println!("========================================");

    // 1. Parse inputs
    let route_input: input::RouteInput = serde_json::from_str(
        &fs::read_to_string(route_json_path).expect("Failed to read route.json")
    ).expect("Failed to parse route.json");

    let stops_input: input::StopsInput = serde_json::from_str(
        &fs::read_to_string(stops_json_path).expect("Failed to read stops.json")
    ).expect("Failed to parse stops.json");

    println!("Loaded {} route points and {} stops", route_input.route_points.len(), stops_input.stops.len());

    // 2. Initial coordinate conversion and deduplication
    let lat_avg = coord::compute_lat_avg(&route_input.route_points.iter().map(|p| (p.lat(), p.lon())).collect::<Vec<_>>());
    println!("Computed average latitude: {:.6}°", lat_avg);
    
    let mut route_pts_cm = Vec::with_capacity(route_input.route_points.len());
    for p in &route_input.route_points {
        let (x, y) = coord::latlon_to_cm_relative(p.lat(), p.lon(), lat_avg);
        let pt = (x as i64, y as i64);
        
        // Deduplicate consecutive identical points
        if let Some(&last_pt) = route_pts_cm.last() {
            if last_pt == pt {
                continue;
            }
        }
        route_pts_cm.push(pt);
    }
    println!("Loaded and deduplicated route: {} -> {} points", route_input.route_points.len(), route_pts_cm.len());

    // Identify indices of route points near stops (±30m protection)
    let mut protected_indices = Vec::new();
    let protection_radius_cm2 = 3000i64 * 3000i64; // 30m radius

    for stop in &stops_input.stops {
        let (sx, sy) = coord::latlon_to_cm_relative(stop.lat, stop.lon, lat_avg);
        
        for (i, p) in route_pts_cm.iter().enumerate() {
            let dx = p.0 - sx as i64;
            let dy = p.1 - sy as i64;
            let d2 = dx*dx + dy*dy;
            if d2 <= protection_radius_cm2 {
                protected_indices.push(i);
            }
        }
    }
    protected_indices.sort_unstable();
    protected_indices.dedup();
    println!("Protected {} points near stops", protected_indices.len());

    // 3. Simplify route (Douglas-Peucker + Curve/Stop/Length Protection)
    let epsilon_general = 700.0; // 7m
    let simplified_pts_cm = simplify::simplify_and_interpolate(&route_pts_cm, epsilon_general, &protected_indices);

    println!("Simplified route: {} -> {} points", route_pts_cm.len(), simplified_pts_cm.len());

    // Debug: Check max segment length in simplified route
    let mut max_len = 0.0;
    for i in 0..simplified_pts_cm.len()-1 {
        let p1 = simplified_pts_cm[i];
        let p2 = simplified_pts_cm[i+1];
        let len = (((p2.0-p1.0).pow(2) + (p2.1-p1.1).pow(2)) as f64).sqrt();
        if len > max_len { max_len = len; }
    }
    println!("Max segment length in simplified route: {:.2} cm", max_len);

    // 4. Linearize route (Compute geometric coefficients)
    let route_nodes = linearize::linearize_route(&simplified_pts_cm);
    println!("Built route graph with {} nodes", route_nodes.len());

    // 5. Project stops (1D progress + non-overlapping corridors)
    let stop_pts_cm: Vec<(i64, i64)> = stops_input.stops.iter().map(|s| {
        let (x, y) = coord::latlon_to_cm_relative(s.lat, s.lon, lat_avg);
        (x as i64, y as i64)
    }).collect();
    
    let projected_stops = stops::project_stops(&stop_pts_cm, &route_nodes);
    println!("Projected {} stops with corridors", projected_stops.len());

    // 6. Build Spatial Grid Index (100m cells)
    let grid_size_cm = 10000; // 100m
    let grid = grid::build_grid(&route_nodes, grid_size_cm);
    println!("Built {}x{} spatial grid ({} cells)", grid.cols, grid.rows, grid.cells.len());

    // 7. Generate LUTs
    let gaussian_lut = lut::generate_gaussian_lut();
    let logistic_lut = lut::generate_logistic_lut();

    // 8. Pack and write binary
    let output_file = fs::File::create(output_bin_path).expect("Failed to create output file");
    pack::pack_v8_route_data(
        &route_nodes,
        &projected_stops,
        &grid,
        &gaussian_lut,
        &logistic_lut,
        &mut &output_file
    ).expect("Failed to pack route data");

    println!("Successfully wrote binary data to {}", output_bin_path);
    println!("========================================");
}
