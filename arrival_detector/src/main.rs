mod input;
mod corridor;

use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: arrival_detector <input.jsonl> <route_data.bin> <output.jsonl>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  input.jsonl     - Phase 2 JSONL output");
        eprintln!("  route_data.bin  - Binary route data from Phase 1");
        eprintln!("  output.jsonl   - Arrival event output");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let route_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);

    println!("Phase 3: Arrival Detection");
    println!("  Input:  {}", input_path.display());
    println!("  Route:  {}", route_path.display());
    println!("  Output: {}", output_path.display());
    println!();
    println!("TODO: Implement pipeline");
}
