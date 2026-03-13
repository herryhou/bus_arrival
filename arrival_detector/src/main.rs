mod input;
mod corridor;
mod probability;
mod state_machine;
mod recovery;
mod output;

use std::env;
use std::path::PathBuf;
use std::fs::File;
use std::io::{BufWriter, Read};

use shared::binfile::RouteData;
use shared::{ArrivalEvent, FsmState};
use state_machine::StopState;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: arrival_detector <input.jsonl> <route_data.bin> <output.jsonl>");
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

    // Load route data
    let mut route_file = File::open(&route_path).expect("Failed to open route_data.bin");
    let mut route_buffer = Vec::new();
    route_file.read_to_end(&mut route_buffer).expect("Failed to read route_data.bin");
    let route_data = RouteData::load(&route_buffer).expect("Failed to parse route_data.bin");

    // Pre-extract stops to avoid redundant unaligned reads
    let mut stops = Vec::with_capacity(route_data.stop_count);
    for i in 0..route_data.stop_count {
        stops.push(route_data.get_stop(i).unwrap());
    }

    // Initialize state
    let mut stop_states: Vec<StopState> = (0..route_data.stop_count as u8)
        .map(|i| StopState::new(i))
        .collect();
    
    let mut last_s_cm = 0;
    let mut current_stop_idx = 0u8;

    let mut output_writer = BufWriter::new(File::create(&output_path).expect("Failed to create output file"));

    // Pre-compute LUTs (Note: we use the ones from route_data if available, 
    // but here we use the builder functions for simplicity as per spec)
    let gaussian_lut = probability::build_gaussian_lut();
    let logistic_lut = probability::build_logistic_lut();

    let mut processed = 0;
    let mut arrivals = 0;

    for record in input::parse_input(&input_path) {
        if !record.valid {
            continue;
        }

        // Check for GPS jump - trigger recovery
        if (record.s_cm - last_s_cm).abs() > 20000 {
            if let Some(new_idx) = recovery::find_stop_index(
                record.s_cm, &stops, current_stop_idx
            ) {
                // If we jumped significantly, reset states for intervening stops
                // (simplified: just update current index)
                current_stop_idx = new_idx as u8;
            }
        }
        last_s_cm = record.s_cm;

        // Find active stops (corridor filter)
        let active_indices = corridor::find_active_stops(record.s_cm, &stops);

        for &stop_idx in &active_indices {
            let stop = &stops[stop_idx];
            let state = &mut stop_states[stop_idx];

            // Handle re-entry after departure
            if state.fsm_state == FsmState::Departed {
                if state.can_reactivate(record.s_cm, stop.progress_cm) {
                    state.reset();
                }
            }

            // Compute probability
            let prob = probability::arrival_probability(
                record.s_cm,
                record.v_cms,
                stop,
                state.dwell_time_s,
                &gaussian_lut,
                &logistic_lut,
            );

            // Update state machine
            if state.update(record.s_cm, record.v_cms, stop.progress_cm, prob) {
                // Just arrived!
                let event = ArrivalEvent {
                    time: record.time,
                    stop_idx: state.index,
                    s_cm: record.s_cm,
                    v_cms: record.v_cms,
                    probability: prob,
                };
                output::write_event(&mut output_writer, &event).expect("Failed to write arrival event");
                arrivals += 1;
                current_stop_idx = state.index;
            }
        }
        processed += 1;
    }

    println!("Processed {} records, detected {} arrivals", processed, arrivals);
}
