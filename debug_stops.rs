// Debug tool to inspect stop corridors in binary file
use std::env;
use std::fs::File;
use std::io::Read;
use std::convert::TryInto;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <binary_file>", args[0]);
        std::process::exit(1);
    }

    let bin_path = &args[1];
    let mut file = File::open(bin_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Parse the binary file format
    // Based on the RouteData structure in shared/src/binfile.rs
    let mut offset = 0;

    // Read header (magic, version, num_nodes, num_stops)
    let magic = u32::from_le_bytes(buffer[offset..offset+4].try_into()?);
    offset += 4;
    println!("Magic: 0x{:08x}", magic);

    let version = u32::from_le_bytes(buffer[offset..offset+4].try_into()?);
    offset += 4;
    println!("Version: {}", version);

    let num_nodes = u32::from_le_bytes(buffer[offset..offset+4].try_into()?);
    offset += 4;
    println!("Num nodes: {}", num_nodes);

    let num_stops = u32::from_le_bytes(buffer[offset..offset+4].try_into()?);
    offset += 4;
    println!("Num stops: {}", num_stops);

    // Skip fixed_origin (16 bytes: 2x f64 for lat, lon)
    offset += 16;

    // Skip route nodes (137 nodes * 52 bytes each)
    println!("Skipping {} route nodes (52 bytes each)...", num_nodes);
    offset += (num_nodes as usize) * 52;

    // Read stops
    println!("\n=== Stop Corridors ===");
    for i in 0..num_stops {
        let progress_cm = i32::from_le_bytes(buffer[offset..offset+4].try_into()?);
        offset += 4;

        let corridor_start_cm = i32::from_le_bytes(buffer[offset..offset+4].try_into()?);
        offset += 4;

        let corridor_end_cm = i32::from_le_bytes(buffer[offset..offset+4].try_into()?);
        offset += 4;

        println!("Stop {}: progress={} cm, corridor=[{}, {}] cm (length={} cm)",
            i, progress_cm, corridor_start_cm, corridor_end_cm,
            corridor_end_cm - corridor_start_cm);
    }

    Ok(())
}
