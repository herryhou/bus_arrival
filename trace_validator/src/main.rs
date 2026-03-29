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

    // Collect all critical issues
    let mut critical_issues: Vec<_> = Vec::new();
    let mut warning_issues: Vec<_> = Vec::new();

    for issue in &result.global_issues {
        match issue.severity {
            trace_validator::Severity::Critical => critical_issues.push(issue),
            trace_validator::Severity::Warning => warning_issues.push(issue),
            _ => {}
        }
    }

    for analysis in result.stops_analyzed.values() {
        for issue in &analysis.issues {
            match issue.severity {
                trace_validator::Severity::Critical => critical_issues.push(issue),
                trace_validator::Severity::Warning => warning_issues.push(issue),
                _ => {}
            }
        }
    }

    if !critical_issues.is_empty() {
        println!("\nCRITICAL ISSUES");
        for issue in &critical_issues {
            if let Some(stop_idx) = issue.stop_idx {
                println!("  [Stop #{}] {}", stop_idx, issue.message);
            } else {
                println!("  - {}", issue.message);
            }
        }
    }

    if !warning_issues.is_empty() && warning_issues.len() <= 10 {
        // Show warnings if there are 10 or fewer
        println!("\nWARNINGS");
        for issue in &warning_issues {
            if let Some(stop_idx) = issue.stop_idx {
                println!("  [Stop #{}] {}", stop_idx, issue.message);
            } else {
                println!("  - {}", issue.message);
            }
        }
    } else if !warning_issues.is_empty() {
        println!("\nWARNINGS: {} issues (see HTML report for details)", warning_issues.len());
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
