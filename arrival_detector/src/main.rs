use std::env;
use std::path::PathBuf;
use std::fs::File;
use std::io::{BufWriter, Read};

use shared::binfile::RouteData;
use shared::{ArrivalEvent, FsmState};
use arrival_detector::state_machine::StopState;
use arrival_detector::{input, corridor, probability, recovery, output, trace};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse arguments: <input.jsonl> <route_data.bin> <output.jsonl> [--trace <trace.jsonl>] [--announce <announce.jsonl>]
    if args.len() != 4 && args.len() != 6 && args.len() != 8 {
        eprintln!("Usage: arrival_detector <input.jsonl> <route_data.bin> <output.jsonl> [--trace <trace.jsonl>] [--announce <announce.jsonl>]");
        std::process::exit(1);
    }

    let enable_trace = args.len() >= 6 && args[4] == "--trace";
    let enable_announce = args.len() == 8 && args[6] == "--announce";

    if args.len() >= 6 && !enable_trace && !enable_announce {
        eprintln!("Error: expected --trace or --announce as 4th argument");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let route_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);
    let trace_path = enable_trace.then(|| PathBuf::from(&args[5]));
    let announce_path = enable_announce.then(|| PathBuf::from(&args[7]));

    println!("Phase 3: Arrival Detection");
    println!("  Input:  {}", input_path.display());
    println!("  Route:  {}", route_path.display());
    println!("  Output: {}", output_path.display());
    if let Some(ref tp) = trace_path {
        println!("  Trace:  {}", tp.display());
    }
    if let Some(ref ap) = announce_path {
        println!("  Announce: {}", ap.display());
    }
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
    let mut trace_writer = trace_path.map(|p| BufWriter::new(File::create(&p).expect("Failed to create trace file")));
    let mut announce_writer = announce_path.map(|p| BufWriter::new(File::create(&p).expect("Failed to create announce file")));

    // Pre-compute LUTs (Note: we use the ones from route_data if available,
    // but here we use the builder functions for simplicity as per spec)
    let gaussian_lut = probability::build_gaussian_lut();
    let logistic_lut = probability::build_logistic_lut();

    let mut processed = 0;
    let mut arrivals = 0;
    let mut announces = 0;

    for record in input::parse_input(&input_path) {
        if !record.valid {
            continue;
        }

        // Check for GPS jump - trigger recovery
        let gps_jump = (record.s_cm - last_s_cm).abs() > 20000;
        let recovery_idx = if gps_jump {
            recovery::find_stop_index(record.s_cm, &stops, current_stop_idx)
        } else {
            None
        };

        if let Some(new_idx) = recovery_idx {
            // If we jumped significantly, reset states for intervening stops
            // (simplified: just update current index)
            current_stop_idx = new_idx as u8;
        }
        last_s_cm = record.s_cm;

        // Find active stops (corridor filter)
        let active_indices = corridor::find_active_stops(record.s_cm, &stops);

        // Track which stops arrived this frame for trace output
        let mut arrived_this_frame: Vec<u8> = Vec::new();

        // Store decision probability and features for each active stop
        // These must be calculated BEFORE state.update() modifies dwell_time_s
        struct StopDecisionData {
            stop_idx: u8,
            probability: u8,
            features: trace::FeatureScores,
        }
        let mut decision_data: Vec<StopDecisionData> = Vec::new();

        for &stop_idx in &active_indices {
            let stop = &stops[stop_idx];
            let state = &mut stop_states[stop_idx];

            // Handle re-entry after departure
            if state.fsm_state == FsmState::Departed {
                if state.can_reactivate(record.s_cm, stop.progress_cm) {
                    state.reset();
                }
            }

            // Compute probability and features BEFORE state.update()
            // These values are used for the state machine decision
            let prob = probability::arrival_probability(
                record.s_cm,
                record.v_cms,
                stop,
                state.dwell_time_s,
                &gaussian_lut,
                &logistic_lut,
            );

            let features = probability::compute_feature_scores(
                record.s_cm,
                record.v_cms,
                stop,
                state.dwell_time_s,
                &gaussian_lut,
                &logistic_lut,
            );

            // Store for trace output (probability used for decision)
            decision_data.push(StopDecisionData {
                stop_idx: stop_idx as u8,
                probability: prob,
                features,
            });

            // Update state machine (may modify dwell_time_s)
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
                arrived_this_frame.push(state.index);
            }

            // v8.4: Check for announcement (corridor entry)
            // Triggers after FSM update to ensure state is correct
            if let Some(ref mut aw) = announce_writer {
                if state.should_announce(record.s_cm, stop.corridor_start_cm) {
                    let announce_event = trace::AnnounceEvent {
                        time: record.time,
                        stop_idx: state.index,
                        s_cm: record.s_cm,
                        v_cms: record.v_cms,
                    };
                    trace::write_announce_event(aw, &announce_event).expect("Failed to write announce event");
                    announces += 1;
                }
            }
        }

        // Write trace record if enabled
        if let Some(ref mut tw) = trace_writer {
            let stop_states: Vec<trace::StopTraceState> = decision_data
                .iter()
                .map(|dd| {
                    let stop = &stops[dd.stop_idx as usize];
                    let state = &stop_states[dd.stop_idx as usize];
                    trace::StopTraceState {
                        stop_idx: dd.stop_idx,
                        distance_cm: record.s_cm - stop.progress_cm,
                        fsm_state: state.fsm_state,
                        dwell_time_s: state.dwell_time_s,
                        // Use the probability that was actually used for decision
                        probability: dd.probability,
                        // Use the features that were actually used for decision
                        features: dd.features.clone(),
                        just_arrived: arrived_this_frame.contains(&dd.stop_idx),
                    }
                })
                .collect();

            let trace_record = trace::TraceRecord {
                time: record.time,
                lat: record.lat,
                lon: record.lon,
                s_cm: record.s_cm,
                v_cms: record.v_cms,
                heading_cdeg: record.heading_cdeg,
                active_stops: active_indices.iter().map(|&i| i as u8).collect(),
                stop_states,
                gps_jump,
                recovery_idx: recovery_idx.map(|i| i as u8),
            };
            trace::write_trace_record(tw, &trace_record).expect("Failed to write trace record");
        }

        processed += 1;
    }

    println!("Processed {} records, detected {} arrivals", processed, arrivals);
    if enable_announce {
        println!("Generated {} announcement events", announces);
    }
}
