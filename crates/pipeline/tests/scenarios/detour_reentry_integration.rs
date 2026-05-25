//! Detour Re-entry Integration Test
//!
//! This test verifies the off-route detection and re-entry snap behavior:
//! 1. During detour: position is frozen, no stop detection
//! 2. After re-entry: position snaps immediately to GPS projection
//! 3. Stops between frozen position and re-entry position are fully skipped

use pipeline::Pipeline;
use shared::binfile::RouteData;
use std::io::BufRead;

use super::common::{load_nmea_reader, load_ty225_route};

/// Test detour scenario with off-route detection and re-entry snap
#[test]
fn test_detour_reentry_snap_behavior() {
    // Load route data
    let route_bytes = load_ty225_route("short_detour");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    // Process NMEA through pipeline
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("short_detour"),
        &route_data,
    )
    .expect("Pipeline processing failed");

    // Read trace to verify off-route behavior
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    let mut off_route_detected = false;
    let mut off_route_start_tick = 0;
    let mut off_route_end_tick: Option<u64> = None;
    let mut frozen_s_cm = 0;
    let mut reentry_tick: Option<u64> = None;
    let mut reentry_s_cm = 0;
    let mut prev_off_route = false;
    let mut awaiting_snap = false;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["gps"]["time_ms"].as_u64().unwrap();
        let s_cm = trace["kalman"]["s_cm"].as_i64().unwrap();
        let off_route = trace["detection"]["off_route"].as_bool().unwrap();

        // Detect off-route episode
        if off_route && !off_route_detected {
            off_route_detected = true;
            off_route_start_tick = time;
            frozen_s_cm = s_cm;
        }

        if off_route_detected && prev_off_route && !off_route && off_route_end_tick.is_none() {
            off_route_end_tick = Some(time);
            awaiting_snap = true;
        }

        if awaiting_snap && !off_route && s_cm != frozen_s_cm {
            reentry_tick = Some(time);
            reentry_s_cm = s_cm;
            awaiting_snap = false;
        }

        prev_off_route = off_route;
    }

    // Verify off-route was detected
    assert!(
        off_route_detected,
        "Off-route episode should be detected in trace"
    );
    let off_route_end_tick = off_route_end_tick.expect("Off-route episode should end in trace");
    let reentry_tick = reentry_tick.expect("Should find re-entry transition in trace");

    // Verify position was frozen during off-route
    let mut frozen_count = 0;
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let off_route = trace["detection"]["off_route"].as_bool().unwrap();
        let s_cm = trace["kalman"]["s_cm"].as_i64().unwrap();

        if off_route {
            frozen_count += 1;
            assert_eq!(
                s_cm, frozen_s_cm,
                "Position should remain frozen at {} during off-route (found {} at tick {})",
                frozen_s_cm, s_cm, trace["gps"]["time_ms"]
            );
        }
    }

    assert!(
        frozen_count > 10,
        "Off-route episode should last for multiple ticks (got {})",
        frozen_count
    );
    assert!(
        off_route_end_tick > off_route_start_tick,
        "Re-entry should happen after off-route detection"
    );

    // Verify re-entry snap happened (position jump from frozen to new position)
    // The re-entry should cause a significant position jump (at least 10m)
    let position_jump = (reentry_s_cm - frozen_s_cm).unsigned_abs();
    assert!(
        position_jump > 1000,
        "Re-entry should snap to new position (jump of {} cm from frozen {} to reentry {})",
        position_jump, frozen_s_cm, reentry_s_cm
    );

    // Verify arrivals - stops 2-5 should be skipped
    let detected_stop_indices: Vec<usize> = result
        .arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // Expected: 0, 1, 6, 7, 8, 9 (stops 2-5 skipped)
    // The re-entry position should be past stop 5, so stops 2-5 are not detected
    let has_stop_2_or_3_or_4_or_5 = detected_stop_indices
        .iter()
        .any(|&idx| idx >= 2 && idx <= 5);

    assert!(
        !has_stop_2_or_3_or_4_or_5,
        "Stops 2-5 should be skipped after detour re-entry. Detected stops: {:?}",
        detected_stop_indices
    );

    // Verify stop 0 is detected (before detour)
    // Note: Stop 1 is NOT detected because off-route is triggered before the dwell completes
    // This is expected behavior - the detour waypoint (stop 6) is ~300m from stop 1,
    // causing off-route detection before stop 1 arrival can be confirmed
    assert!(
        detected_stop_indices.contains(&0),
        "Stop 0 should be detected before detour"
    );

    // Verify stops 6+ are detected (after re-entry)
    let has_stops_after_reentry = detected_stop_indices.iter().any(|&idx| idx >= 6);
    assert!(
        has_stops_after_reentry,
        "Stops 6+ should be detected after detour re-entry"
    );

    println!("Detour re-entry test passed:");
    println!("  Off-route detected at tick: {}", off_route_start_tick);
    println!("  Frozen position: {} cm", frozen_s_cm);
    println!("  Off-route duration: {} ticks", frozen_count);
    println!("  Re-entry at tick: {}", reentry_tick);
    println!("  Re-entry position: {} cm", reentry_s_cm);
    println!("  Position jump: {} cm", position_jump);
    println!("  Detected stops: {:?}", detected_stop_indices);
}

/// Test that arrivals are NOT triggered during off-route episode
#[test]
fn test_no_arrivals_during_offroute() {
    // Load route data
    let route_bytes = load_ty225_route("short_detour");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    // Process NMEA through pipeline
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("short_detour"),
        &route_data,
    )
    .expect("Pipeline processing failed");

    // Read trace to find off-route episode time range
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    let mut off_route_start_time: Option<u64> = None;
    let mut off_route_end_time: Option<u64> = None;

    // First pass: find off-route episode timing
    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["gps"]["time_ms"].as_u64().unwrap();
        let off_route = trace["detection"]["off_route"].as_bool().unwrap();

        if off_route && off_route_start_time.is_none() {
            off_route_start_time = Some(time);
        }
        if !off_route && off_route_start_time.is_some() && off_route_end_time.is_none() {
            off_route_end_time = Some(time);
        }
    }

    assert!(
        off_route_start_time.is_some(),
        "Off-route episode should be detected"
    );
    assert!(
        off_route_end_time.is_some(),
        "Off-route episode should end"
    );

    // Verify no arrivals occurred during off-route
    for arrival in &result.arrivals {
        assert!(
            arrival.time < off_route_start_time.unwrap() || arrival.time >= off_route_end_time.unwrap(),
            "Arrival at time {} (stop {}) should not occur during off-route episode ({:?})",
            arrival.time, arrival.stop_idx,
            (off_route_start_time, off_route_end_time)
        );
    }

    println!("Verified: No arrivals during off-route episode ({:?})",
             (off_route_start_time, off_route_end_time));
}

/// Test that re-entry uses immediate snap, not gradual catch-up
#[test]
fn test_reentry_immediate_snap_not_gradual() {
    // Load route data
    let route_bytes = load_ty225_route("short_detour");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    // Process NMEA through pipeline
    let _result = Pipeline::process_nmea_reader(
        load_nmea_reader("short_detour"),
        &route_data,
    )
    .expect("Pipeline processing failed");

    // Read trace to verify immediate snap behavior
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    let mut frozen_s_cm: Option<i64> = None;
    let mut off_route_end_tick: Option<u64> = None;
    let mut reentry_transition: Option<(u64, i64, u64, i64)> = None;
    let mut prev_off_route = false;
    let mut awaiting_snap = false;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["gps"]["time_ms"].as_u64().unwrap();
        let s_cm = trace["kalman"]["s_cm"].as_i64().unwrap();
        let off_route = trace["detection"]["off_route"].as_bool().unwrap();

        if off_route && frozen_s_cm.is_none() {
            frozen_s_cm = Some(s_cm);
        }

        if prev_off_route && !off_route && off_route_end_tick.is_none() {
            off_route_end_tick = Some(time);
            awaiting_snap = true;
        }

        if awaiting_snap {
            if let Some(frozen_s_cm) = frozen_s_cm {
                if !off_route && s_cm != frozen_s_cm {
                    reentry_transition = Some((off_route_end_tick.unwrap(), frozen_s_cm, time, s_cm));
                    break;
                }
            }
        }

        prev_off_route = off_route;
    }

    let (off_route_tick, frozen_s_cm, reentry_tick, reentry_s_cm) =
        reentry_transition.expect("Should find re-entry transition in trace");
    assert!(
        reentry_tick >= off_route_tick,
        "Re-entry should happen after off-route detection"
    );

    // Check for significant jump (not gradual)
    let jump = (reentry_s_cm - frozen_s_cm).unsigned_abs();
    assert!(
        jump > 10000, // At least 100m jump indicates immediate snap
        "Re-entry should immediately snap to new position (jump of {} cm from {} to {})",
        jump, frozen_s_cm, reentry_s_cm
    );

    println!("Re-entry immediate snap verified:");
    println!("  Frozen position: {} cm", frozen_s_cm);
    println!("  Re-entry position: {} cm (jump: {} cm)", reentry_s_cm, jump);
}
