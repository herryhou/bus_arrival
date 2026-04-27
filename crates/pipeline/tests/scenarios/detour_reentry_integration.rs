//! Detour Re-entry Integration Test
//!
//! This test verifies the off-route detection and re-entry snap behavior:
//! 1. During detour: position is frozen, no stop detection
//! 2. After re-entry: position snaps immediately to GPS projection
//! 3. Stops between frozen position and re-entry position are fully skipped

use pipeline::Pipeline;
use pipeline::PipelineConfig;
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
        &PipelineConfig::default(),
    )
    .expect("Pipeline processing failed");

    // Read trace to verify off-route behavior
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    let mut off_route_detected = false;
    let mut off_route_start_tick = 0;
    let mut frozen_s_cm = 0;
    let mut reentry_tick = 0;
    let mut reentry_s_cm = 0;
    let mut s_cm_before_offroute = 0;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["time"].as_u64().unwrap();
        let s_cm = trace["s_cm"].as_i64().unwrap();
        let off_route = trace["off_route"].as_bool().unwrap();

        // Detect off-route episode
        if off_route && !off_route_detected {
            off_route_detected = true;
            off_route_start_tick = time;
            frozen_s_cm = s_cm;
            // Record s_cm before off-route (should be from previous tick)
            if let Ok(trace_file_full) = std::fs::File::open("test_data/ty225_short_detour_trace.jsonl")
            {
                // We'll capture the pre-off-route s_cm from the first off-route tick
            }
        }

        // Detect re-entry (transition from off_route=true to off_route=false)
        if off_route_detected && reentry_tick == 0 && !off_route {
            reentry_tick = time;
            reentry_s_cm = s_cm;
        }
    }

    // Verify off-route was detected
    assert!(
        off_route_detected,
        "Off-route episode should be detected in trace"
    );

    // Verify position was frozen during off-route
    let mut frozen_count = 0;
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let off_route = trace["off_route"].as_bool().unwrap();
        let s_cm = trace["s_cm"].as_i64().unwrap();

        if off_route {
            frozen_count += 1;
            assert_eq!(
                s_cm, frozen_s_cm,
                "Position should remain frozen at {} during off-route (found {} at tick {})",
                frozen_s_cm, s_cm, trace["time"]
            );
        }
    }

    assert!(
        frozen_count > 10,
        "Off-route episode should last for multiple ticks (got {})",
        frozen_count
    );

    // Verify re-entry snap happened (position jump from frozen to new position)
    assert!(
        reentry_tick > off_route_start_tick,
        "Re-entry should happen after off-route detection"
    );

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

    // Verify stops 0, 1 are detected (before detour)
    assert!(
        detected_stop_indices.contains(&0),
        "Stop 0 should be detected before detour"
    );
    assert!(
        detected_stop_indices.contains(&1),
        "Stop 1 should be detected before detour"
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
        &PipelineConfig::default(),
    )
    .expect("Pipeline processing failed");

    // Read trace to find off-route episode time range
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    let mut off_route_start_time: Option<u64> = None;
    let mut off_route_end_time: Option<u64> = None;

    // First pass: find off-route episode timing
    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["time"].as_u64().unwrap();
        let off_route = trace["off_route"].as_bool().unwrap();

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
            arrival.time < off_route_start_time.unwrap() || arrival.time > off_route_end_time.unwrap(),
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
        &PipelineConfig::default(),
    )
    .expect("Pipeline processing failed");

    // Read trace to verify immediate snap behavior
    let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
        .expect("Failed to open trace file");
    let trace_reader = std::io::BufReader::new(trace_file);

    let mut ticks = Vec::new();

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["time"].as_u64().unwrap();
        let s_cm = trace["s_cm"].as_i64().unwrap();
        let off_route = trace["off_route"].as_bool().unwrap();

        ticks.push((time, s_cm, off_route));
    }

    // Find off-route to re-entry transition
    let mut reentry_idx = 0;
    for (i, (_, s_cm, off_route)) in ticks.iter().enumerate() {
        if !off_route && i > 0 && ticks[i - 1].2 {
            // Found first non-off-route tick after off-route
            reentry_idx = i;
            break;
        }
    }

    assert!(
        reentry_idx > 0,
        "Should find re-entry transition in trace"
    );

    // The key check: after re-entry, s_cm should NOT gradually increase
    // from the frozen position. Instead, it should jump immediately.
    let frozen_s_cm = ticks[reentry_idx - 1].1;
    let reentry_s_cm = ticks[reentry_idx].1;

    // Check for significant jump (not gradual)
    let jump = (reentry_s_cm - frozen_s_cm).unsigned_abs();
    assert!(
        jump > 10000, // At least 100m jump indicates immediate snap
        "Re-entry should immediately snap to new position (jump of {} cm from {} to {})",
        jump, frozen_s_cm, reentry_s_cm
    );

    // Verify no gradual catch-up: check next few ticks
    // The position should advance normally from the reentry position,
    // not gradually from the frozen position
    if reentry_idx + 3 < ticks.len() {
        let post_reentry_s_cm_1 = ticks[reentry_idx + 1].1;
        let post_reentry_s_cm_2 = ticks[reentry_idx + 2].1;
        let post_reentry_s_cm_3 = ticks[reentry_idx + 3].1;

        // Verify s_cm is increasing (bus moving forward)
        assert!(
            post_reentry_s_cm_1 >= reentry_s_cm - 1000, // Allow small backward movement due to GPS noise
            "Position after re-entry should not drop significantly ({} -> {})",
            reentry_s_cm, post_reentry_s_cm_1
        );

        // Verify normal progression (not catching up from frozen position)
        // If gradual catch-up was happening, we'd see s_cm moving from frozen towards reentry
        // With immediate snap, s_cm should already be at reentry position and moving forward
        let movement_1 = (post_reentry_s_cm_1 - reentry_s_cm).unsigned_abs();
        let movement_2 = (post_reentry_s_cm_2 - post_reentry_s_cm_1).unsigned_abs();
        let movement_3 = (post_reentry_s_cm_3 - post_reentry_s_cm_2).unsigned_abs();

        // Normal movement should be consistent (bus speed)
        // If we were catching up, movement_1 would be very large
        assert!(
            movement_1 < 100000, // Less than 1km per second is reasonable
            "Movement after re-entry should be normal bus speed, not catch-up ({} cm)",
            movement_1
        );

        println!("Re-entry immediate snap verified:");
        println!("  Frozen position: {} cm", frozen_s_cm);
        println!("  Re-entry position: {} cm (jump: {} cm)", reentry_s_cm, jump);
        println!("  Post-reentry movement: {}, {}, {} cm",
                 movement_1, movement_2, movement_3);
    }
}
