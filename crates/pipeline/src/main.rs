//! Bus Arrival Detection Pipeline Binary
//!
//! Single binary that processes NMEA → Arrivals/Departures directly.
//! Wraps the complete pipeline library.

#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

#[cfg(feature = "std")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use pipeline::Pipeline;
    use std::io::Write;

    let args = parse_args()?;

    // Determine trace output path
    let trace_path = args.output.unwrap_or_else(|| {
        let auto_path = generate_trace_path(&args.input);
        eprintln!("Auto-generating trace output: {}", auto_path.display());
        auto_path
    });

    // Run pipeline
    let result = Pipeline::process_file(
        &args.input,
        &args.route_data,
    )?;

    // Write trace file
    use std::io::BufWriter;
    let file = std::fs::File::create(&trace_path)?;
    let mut writer = BufWriter::new(file);
    for trace_record in &result.trace_records {
        writeln!(writer, "{}", serde_json::to_string(trace_record)?)?;
    }
    writer.flush()?;
    eprintln!("Trace written to: {}", trace_path.display());

    // Print summary
    eprintln!("=== Pipeline Complete ===");
    eprintln!("Processed {} GPS updates", result.trace_records.len());
    eprintln!("Detected {} arrivals", result.arrivals.len());
    eprintln!("Detected {} departures", result.departures.len());

    Ok(())
}

#[cfg(feature = "std")]
struct Args {
    input: PathBuf,
    route_data: PathBuf,
    output: Option<PathBuf>,
}

#[cfg(feature = "std")]
fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut input = None;
    let mut route_data = None;
    let mut output = None;

    let mut args_iter = std::env::args().skip(1);

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--output" => {
                if let Some(output_path) = args_iter.next() {
                    output = Some(PathBuf::from(output_path));
                } else {
                    return Err("--output requires an argument".into());
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
                // Positional arguments: input route_data
                if input.is_none() {
                    input = Some(PathBuf::from(arg));
                } else if route_data.is_none() {
                    route_data = Some(PathBuf::from(arg));
                } else {
                    return Err("Too many arguments. Usage: pipeline <input> <route_data> [--output <trace_v2.jsonl>]".into());
                }
            }
        }
    }

    let input = input.ok_or("Missing input file")?;
    let route_data = route_data.ok_or("Missing route_data.bin file")?;

    Ok(Args {
        input,
        route_data,
        output,
    })
}

#[cfg(feature = "std")]
fn print_help() {
    println!("Bus Arrival Detection Pipeline");
    println!();
    println!("Usage: pipeline [OPTIONS] <input> <route_data>");
    println!();
    println!("Arguments:");
    println!("  <input>      NMEA or JSONL GPS log file");
    println!("  <route_data> Route data binary file");
    println!();
    println!("Options:");
    println!("  --output <file>    Trace output file (default: auto-generated from input name)");
    println!("  -h, --help         Show this help message");
    println!();
    println!("Examples:");
    println!("  pipeline gps.nmea route_data.bin");
    println!("  pipeline gps.jsonl route_data.bin --output custom_trace_v2.jsonl");
    println!();
    println!("Helper scripts:");
    println!("  ./tools/arrival_from_trace.sh trace_v2.jsonl > arrivals.jsonl");
    println!("  ./tools/announce_from_trace.sh trace_v2.jsonl > announce.jsonl");
}

/// Generate trace output path from NMEA input path
/// Example: test_data/ty225_normal_nmea.txt -> test_data/ty225_normal_trace_v2.jsonl
#[cfg(feature = "std")]
fn generate_trace_path(nmea_path: &Path) -> PathBuf {
    let mut trace_path = nmea_path.to_path_buf();

    // Replace extension with _trace_v2.jsonl
    let file_stem = trace_path.file_stem().unwrap_or_default();
    let parent = trace_path.parent();

    // Strip _nmea suffix if present
    let stem_str = file_stem.to_string_lossy();
    let base_name = stem_str.strip_suffix("_nmea").unwrap_or(&stem_str);

    let new_name = format!("{}_trace_v2.jsonl", base_name);

    if let Some(p) = parent {
        trace_path = p.join(new_name);
    } else {
        trace_path = PathBuf::from(new_name);
    }

    trace_path
}
