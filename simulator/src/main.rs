mod route_data;
mod nmea;
mod grid;
mod map_match;
mod kalman;
mod output;

use std::env;
use std::path::PathBuf;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: simulator <nmea_file> <route_data.bin> <output.jsonl>");
        std::process::exit(1);
    }

    let nmea_path = PathBuf::from(&args[1]);
    let route_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);

    println!("Phase 2: Localization Pipeline");
    println!("  NMEA input:   {}", nmea_path.display());
    println!("  Route data:   {}", route_path.display());
    println!("  Output:       {}", output_path.display());
    println!();

    // Load route data
    let route_data = route_data::load_route_data(&route_path)
        .expect("Failed to load route_data.bin");
    println!("Loaded {} nodes, {} stops", route_data.nodes.len(), route_data.stops.len());

    // Initialize state
    let mut kalman = shared::KalmanState::new();
    let mut dr = shared::DrState::new();
    let mut nmea_state = nmea::NmeaState::new();

    // Open NMEA file and output
    let nmea_file = File::open(&nmea_path).expect("Failed to open NMEA file");
    let reader = BufReader::new(nmea_file);
    let mut output = BufWriter::new(File::create(&output_path).expect("Failed to create output"));

    let mut time = 0u64;
    let mut processed = 0;

    // Process each line
    for line in reader.lines() {
        let line = line.expect("Failed to read line");

        if let Some(gps) = nmea_state.parse_sentence(&line) {
            let result = kalman::process_gps_update(&mut kalman, &mut dr, &gps, &route_data, time);
            output::write_output(&mut output, time, &result).expect("Failed to write output");
            processed += 1;
        }

        time += 1;
    }

    output.flush().expect("Failed to flush output");
    println!("Processed {} GPS updates", processed);
}
