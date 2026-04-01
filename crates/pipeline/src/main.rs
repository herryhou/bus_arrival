//! Bus Arrival Detection Pipeline Binary
//!
//! Single binary that processes NMEA → Arrivals/Departures directly.
//! Wraps the complete pipeline library.

#[cfg(feature = "std")]
use std::path::PathBuf;

#[cfg(feature = "std")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use pipeline::{Pipeline, PipelineConfig};
    use std::io::Write;

    let args = parse_args()?;

    // Build configuration
    let mut config = PipelineConfig::default();
    config.enable_trace = args.trace.is_some();
    config.enable_announce = args.announce.is_some();

    // Run pipeline
    let result = Pipeline::process_nmea_file(
        &args.nmea,
        &args.route_data,
        &args.output,
        &config,
    )?;

    // Write trace if requested
    if let Some(trace_path) = args.trace {
        use std::io::BufWriter;
        let file = std::fs::File::create(&trace_path)?;
        let mut writer = BufWriter::new(file);
        for trace_record in result.trace_records.as_ref().unwrap() {
            writeln!(writer, "{}", serde_json::to_string(trace_record)?)?;
        }
        writer.flush()?;
        eprintln!("Trace written to: {}", trace_path.display());
    }

    // Write announce if requested
    if let Some(announce_path) = args.announce {
        use std::io::BufWriter;
        let file = std::fs::File::create(&announce_path)?;
        let mut writer = BufWriter::new(file);
        for announce_event in result.announce_events.as_ref().unwrap() {
            writeln!(writer, "{}", serde_json::to_string(announce_event)?)?;
        }
        writer.flush()?;
        eprintln!("Announce events written to: {}", announce_path.display());
    }

    // Print summary
    eprintln!("=== Pipeline Complete ===");
    eprintln!("Processed {} GPS updates", result.trace_records.as_ref().map(|t| t.len()).unwrap_or(0));
    eprintln!("Detected {} arrivals", result.arrivals.len());
    eprintln!("Detected {} departures", result.departures.len());
    if config.enable_announce {
        eprintln!("Generated {} announce events", result.announce_events.as_ref().unwrap().len());
    }

    Ok(())
}

#[cfg(feature = "std")]
struct Args {
    nmea: PathBuf,
    route_data: PathBuf,
    output: PathBuf,
    trace: Option<PathBuf>,
    announce: Option<PathBuf>,
}

#[cfg(feature = "std")]
fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut nmea = None;
    let mut route_data = None;
    let mut output = None;
    let mut trace = None;
    let mut announce = None;

    let mut args_iter = std::env::args().skip(1);

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--trace" => {
                if let Some(trace_path) = args_iter.next() {
                    trace = Some(PathBuf::from(trace_path));
                } else {
                    return Err("--trace requires an argument".into());
                }
            }
            "--announce" => {
                if let Some(announce_path) = args_iter.next() {
                    announce = Some(PathBuf::from(announce_path));
                } else {
                    return Err("--announce requires an argument".into());
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg).into());
            }
            _ => {
                // Positional arguments: nmea route_data output
                if nmea.is_none() {
                    nmea = Some(PathBuf::from(arg));
                } else if route_data.is_none() {
                    route_data = Some(PathBuf::from(arg));
                } else if output.is_none() {
                    output = Some(PathBuf::from(arg));
                } else {
                    return Err("Too many arguments".into());
                }
            }
        }
    }

    let nmea = nmea.ok_or("Missing NMEA input file")?;
    let route_data = route_data.ok_or("Missing route_data.bin file")?;
    let output = output.ok_or("Missing output file")?;

    Ok(Args {
        nmea,
        route_data,
        output,
        trace,
        announce,
    })
}

#[cfg(feature = "std")]
fn print_help() {
    println!("Bus Arrival Detection Pipeline");
    println!();
    println!("Usage: pipeline [OPTIONS] <nmea> <route_data> <output>");
    println!();
    println!("Arguments:");
    println!("  <nmea>       NMEA log file (GPS data)");
    println!("  <route_data> Route data binary file");
    println!("  <output>     Output JSONL file for arrivals/departures");
    println!();
    println!("Options:");
    println!("  --trace <file>    Enable trace output to file (for debugging)");
    println!("  --announce <file> Enable announce event output to file");
    println!("  -h, --help        Show this help message");
    println!();
    println!("Example:");
    println!("  pipeline gps.nmea route_data.bin arrivals.jsonl --trace trace.jsonl");
}
