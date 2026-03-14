use std::env;
use std::path::PathBuf;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use simulator::{route_data, nmea, kalman, output};

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

    // Load route data into memory buffer
    let mut route_file = File::open(&route_path).expect("Failed to open route_data.bin");
    let mut route_buffer = Vec::new();
    use std::io::Read;
    route_file.read_to_end(&mut route_buffer).expect("Failed to read route_data.bin");

    // Zero-copy load
    let route_data = route_data::RouteData::load(&route_buffer)
        .expect("Failed to load route_data.bin");
    println!("Loaded {} nodes, {} stops", route_data.node_count, route_data.stop_count);


    // Initialize state
    let mut kalman = shared::KalmanState::new();
    let mut dr = shared::DrState::new();
    let mut nmea_state = nmea::NmeaState::new();

    // Open NMEA file and output
    let nmea_file = File::open(&nmea_path).expect("Failed to open NMEA file");
    let reader = BufReader::new(nmea_file);
    let mut output_writer = BufWriter::new(File::create(&output_path).expect("Failed to create output"));

    let mut time = 0u64;
    let mut processed = 0;
    let mut is_first_fix = true;

    // Process each line
    for line in reader.lines() {
        let line = line.expect("Failed to read line");

        if let Some(mut gps) = nmea_state.parse_sentence(&line) {
            gps.timestamp = time;
            let result = kalman::process_gps_update(&mut kalman, &mut dr, &gps, &route_data, time, is_first_fix);
            output::write_output(&mut output_writer, time, gps.lat, gps.lon, gps.heading_cdeg, &result, &route_data).expect("Failed to write output");
            processed += 1;
            
            if is_first_fix {
                is_first_fix = false;
            }
        }

        time += 1;
    }

    output_writer.flush().expect("Failed to flush output");
    println!("Processed {} GPS updates", processed);
}
