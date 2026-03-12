mod route_data;
mod nmea;
mod grid;
mod map_match;

use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: simulator <nmea_file> <route_data.bin> <output.jsonl>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  nmea_file       - NMEA input file");
        eprintln!("  route_data.bin  - Binary route data from Phase 1");
        eprintln!("  output.jsonl   - JSON output (one line per GPS update)");
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
    println!("TODO: Implement pipeline");
}
