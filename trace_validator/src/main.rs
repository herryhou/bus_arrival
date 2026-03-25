use clap::Parser;
use trace_validator::{Analyzer, ReportGenerator, TraceParser, Validator};

#[derive(Parser)]
#[command(name = "trace_validator")]
#[command(about = "Validate bus arrival detection traces", long_about = None)]
struct Args {
    /// Trace file to analyze (.jsonl format)
    trace_file: String,

    /// Optional ground truth file for dwell time comparison
    #[arg(short, long)]
    ground_truth: Option<String>,

    /// Output HTML report path
    #[arg(short, long, default_value = "report.html")]
    output: String,

    /// Verbose console output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.verbose {
        println!("Analyzing trace: {}", args.trace_file);
        if let Some(ref gt) = args.ground_truth {
            println!("Ground truth: {}", gt);
        }
    }

    let records = TraceParser::parse_trace(std::path::Path::new(&args.trace_file))?;

    let ground_truth = if let Some(ref gt_path) = args.ground_truth {
        Some(TraceParser::parse_ground_truth(std::path::Path::new(gt_path))?)
    } else {
        None
    };

    let mut result = Analyzer::analyze(records);
    result.trace_file = args.trace_file.clone();

    Validator::validate(&mut result, ground_truth.as_ref());

    ReportGenerator::generate(&result, std::path::PathBuf::from(&args.output))?;

    if args.verbose {
        print_summary(&result);
        println!("\nHTML report generated: {}", args.output);
    }

    Ok(())
}

fn print_summary(result: &trace_validator::ValidationResult) {
    println!("\nSUMMARY");
    println!("  Total records:     {}", result.total_records);
    println!("  Time range:        {}..{} ({}s)",
             result.time_range.0, result.time_range.1,
             result.time_range.1 - result.time_range.0);
    println!("  Stops analyzed:    {}", result.total_stops());
    println!("  With AtStop:       {}/{}", result.stops_with_at_stop(), result.total_stops());
    println!("  GPS jumps:         {}", result.gps_jump_count);

    if !result.global_issues.is_empty() {
        println!("\nGLOBAL ISSUES");
        for issue in &result.global_issues {
            println!("  - {}", issue.message);
        }
    }

    let health = result.stops_with_at_stop() * 100 / result.total_stops();
    println!();
    match health {
        h if h >= 95 => println!("SYSTEM HEALTH: EXCELLENT ({}%)", h),
        h if h >= 80 => println!("SYSTEM HEALTH: GOOD ({}%)", h),
        h if h >= 50 => println!("SYSTEM HEALTH: FAIR ({}%)", h),
        h => println!("SYSTEM HEALTH: POOR ({}%)", h),
    }
}
