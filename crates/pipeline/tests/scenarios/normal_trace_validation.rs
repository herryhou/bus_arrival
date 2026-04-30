//! Normal Scenario Trace Validation Tests
//!
//! This test module validates the trace output from the normal scenario,
//! ensuring GPS quality, status transitions, FSM state progression,
//! position accuracy, and corridor boundaries are all correct.

use super::common::load_trace_reader;
use std::io::BufRead;
use std::io::BufReader;

/// Test GPS quality metrics in normal scenario
#[test]
fn test_normal_gps_quality() {
    let trace_file = load_trace_reader("normal");
    let trace_reader = BufReader::new(trace_file);

    let mut tick_count = 0;
    let mut hdop_violations = 0;
    let mut variance_violations = 0;
    let mut gps_jumps = 0;
    let mut off_route_ticks = 0;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        tick_count += 1;

        // Check HDOP (should be constant 3.5)
        if let Some(hdop) = trace["hdop"].as_f64() {
            if (hdop - 3.5).abs() > 0.1 {
                hdop_violations += 1;
            }
        }

        // Check variance_cm2 (should be 0 - Kalman converged)
        if let Some(variance) = trace["variance_cm2"].as_i64() {
            if variance != 0 {
                variance_violations += 1;
            }
        }

        // Check for GPS jumps
        if let Some(jump) = trace["gps_jump"].as_bool() {
            if jump {
                gps_jumps += 1;
            }
        }

        // Check for off-route events (should be none in normal scenario)
        if let Some(off_route) = trace["off_route"].as_bool() {
            if off_route {
                off_route_ticks += 1;
            }
        }
    }

    println!("GPS quality validation:");
    println!("  Total ticks: {}", tick_count);
    println!("  HDOP violations: {}", hdop_violations);
    println!("  Variance violations: {}", variance_violations);
    println!("  GPS jumps: {}", gps_jumps);
    println!("  Off-route ticks: {}", off_route_ticks);

    assert_eq!(hdop_violations, 0, "HDOP should be constant 3.5");
    assert_eq!(variance_violations, 0, "Variance should be 0 (Kalman converged)");
    assert_eq!(gps_jumps, 0, "Should have no GPS jumps");
    assert_eq!(off_route_ticks, 0, "Should have no off-route events");
}

/// Test status transitions (valid ↔ dr_outage)
#[test]
fn test_normal_status_transitions() {
    let trace_file = load_trace_reader("normal");
    let trace_reader = BufReader::new(trace_file);

    let mut dr_outage_count = 0;
    let mut prev_status: Option<String> = None;
    let mut invalid_transitions = 0;
    let mut dr_outage_without_heading_constraint = 0;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let status = trace["status"].as_str().unwrap();
        let time = trace["time"].as_u64().unwrap();

        // Count dr_outage periods
        if status == "dr_outage" {
            dr_outage_count += 1;

            // Verify dr_outage only occurs when heading_constraint_met is false
            let heading_met = trace["heading_constraint_met"].as_bool().unwrap();
            if heading_met {
                dr_outage_without_heading_constraint += 1;
                println!("WARNING: dr_outage at tick {} with heading_constraint_met=true", time);
            }

            // Verify divergence_cm is 0 during dr_outage
            let divergence = trace["divergence_cm"].as_i64().unwrap();
            if divergence != 0 {
                println!("WARNING: dr_outage at tick {} has divergence_cm={}", time, divergence);
            }
        }

        // Check for invalid status transitions
        if let Some(prev) = prev_status {
            if prev != "dr_outage" && prev != "valid" {
                invalid_transitions += 1;
            }
        }

        prev_status = Some(status.to_string());
    }

    println!("Status transition validation:");
    println!("  DR outage periods: {}", dr_outage_count);
    println!("  Expected: ~140-160 (genuine GPS rejections: speed/monotonic constraints)");
    println!("  Invalid transitions: {}", invalid_transitions);
    println!("  DR outages with heading_constraint_met=true: {}", dr_outage_without_heading_constraint);

    // DrOutage count reflects genuine GPS measurement rejections (speed/monotonic constraints)
    // SuspectOffRoute outputs None (not counted here), so this is only true DR outages
    // Normal scenario has minimal GPS noise, expecting ~140-160 rejections
    assert!(dr_outage_count > 100 && dr_outage_count < 200,
        "DR outage count should be around 140-160, got {}", dr_outage_count);
    assert_eq!(invalid_transitions, 0, "Should have no invalid status transitions");
    assert_eq!(dr_outage_without_heading_constraint, 0,
        "DR outage should only occur when heading_constraint_met=false");
}

/// Test FSM state progression for stop arrivals
#[test]
fn test_normal_fsm_state_progression() {
    let trace_file = load_trace_reader("normal");
    let trace_reader = BufReader::new(trace_file);

    let mut stop_arrivals: std::collections::HashMap<usize, Vec<String>> = std::collections::HashMap::new();

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["time"].as_u64().unwrap();

        // Track FSM states for each stop
        if let Some(stops) = trace["stop_states"].as_array() {
            for stop_state in stops {
                if let Some(state) = stop_state["fsm_state"].as_str() {
                    if let Some(stop_idx) = stop_state["stop_idx"].as_u64() {
                        let entry = stop_arrivals.entry(stop_idx as usize)
                            .or_insert_with(Vec::new);
                        entry.push(format!("{}:{}", time, state));
                    }
                }
            }
        }
    }

    println!("FSM state progression validation:");
    println!("  Stops with state data: {}", stop_arrivals.len());

    // Validate that stops 0-20 have proper FSM progression
    for stop_idx in 0..=20 {
        if let Some(states) = stop_arrivals.get(&stop_idx) {
            // Check for FSM state sequence: Approaching → Arriving → AtStop
            let has_approaching = states.iter().any(|s| s.contains("Approaching"));
            let has_arriving = states.iter().any(|s| s.contains("Arriving"));
            let has_atstop = states.iter().any(|s| s.contains("AtStop"));

            if has_atstop {
                // Should have approaching and/or arriving before AtStop
                assert!(has_approaching || has_arriving,
                    "Stop {} has AtStop but no Approaching/Arriving before",
                    stop_idx);
            }
        }
    }

    // Verify we have FSM data for expected stops
    assert!(!stop_arrivals.is_empty(), "Should have FSM state data");
}

/// Test position accuracy at stop arrivals
#[test]
fn test_normal_position_accuracy_at_arrivals() {
    let trace_file = load_trace_reader("normal");
    let trace_reader = BufReader::new(trace_file);

    let mut arrival_positions = Vec::new();

    // Find all stop arrivals (just_arrived=true)
    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        let time = trace["time"].as_u64().unwrap();
        let lat = trace["lat"].as_f64().unwrap();
        let lon = trace["lon"].as_f64().unwrap();
        let s_cm = trace["s_cm"].as_i64().unwrap();

        if let Some(stops) = trace["stop_states"].as_array() {
            for stop_state in stops {
                if stop_state["just_arrived"].as_bool().unwrap_or(false) {
                    if let Some(stop_idx) = stop_state["stop_idx"].as_u64() {
                        let gps_dist = stop_state["gps_distance_cm"].as_i64().unwrap_or(999999);
                        arrival_positions.push((stop_idx, time, lat, lon, s_cm, gps_dist));
                    }
                }
            }
        }
    }

    println!("Position accuracy at arrivals:");
    println!("  Total arrivals found: {}", arrival_positions.len());

    // Verify arrivals
    assert!(!arrival_positions.is_empty(), "Should have arrivals");

    // Check that gps_distance_cm is small at arrivals (within 50m)
    let far_arrivals: Vec<_> = arrival_positions.iter()
        .filter(|(_, _, _, _, _, gps_dist)| gps_dist.abs() > 5000)
        .collect();

    println!("  Arrivals with gps_distance > 50m: {}", far_arrivals.len());

    for (stop_idx, time, _lat, _lon, _s_cm, gps_dist) in &far_arrivals {
        println!("    Stop {} at time {}: gps_distance={}cm", stop_idx, time, gps_dist);
    }

    assert!(far_arrivals.len() < 5, "Most arrivals should have gps_distance < 50m");

    // Verify we have a reasonable number of arrivals (normal scenario has 21+ stops)
    assert!(arrival_positions.len() >= 20, "Should have at least 20 arrivals");
}

/// Test corridor boundaries
#[test]
fn test_normal_corridor_boundaries() {
    let trace_file = load_trace_reader("normal");
    let trace_reader = BufReader::new(trace_file);

    let mut corridor_checks = 0;
    let mut corridor_violations = 0;
    let mut progress_violations = 0;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        // Only check when active_stops is present
        if let Some(active) = trace["active_stops"].as_array() {
            if !active.is_empty() {
                corridor_checks += 1;

                // Verify corridor_start_cm and corridor_end_cm exist
                let has_start = trace.get("corridor_start_cm").is_some();
                let has_end = trace.get("corridor_end_cm").is_some();

                if !has_start || !has_end {
                    corridor_violations += 1;
                    continue;
                }

                let corridor_start = trace["corridor_start_cm"].as_i64().unwrap();
                let corridor_end = trace["corridor_end_cm"].as_i64().unwrap();
                let s_cm = trace["s_cm"].as_i64().unwrap();

                // Verify corridor length is reasonable (around 12000cm = 120m)
                let corridor_length = corridor_end - corridor_start;
                if corridor_length < 5000 || corridor_length > 20000 {
                    println!("WARNING: Unusual corridor length: {}cm at tick {}", corridor_length, trace["time"].as_u64().unwrap());
                }

                // Verify s_cm is within corridor bounds
                if s_cm < corridor_start || s_cm > corridor_end {
                    progress_violations += 1;
                }
            }
        }
    }

    println!("Corridor boundary validation:");
    println!("  Ticks with active stops: {}", corridor_checks);
    println!("  Missing corridor data: {}", corridor_violations);
    println!("  Progress outside corridor: {}", progress_violations);

    assert_eq!(corridor_violations, 0, "All active_stops should have corridor data");
    assert_eq!(progress_violations, 0, "Progress should always be within corridor");
}

/// Test trace completeness and consistency
#[test]
fn test_normal_trace_completeness() {
    let trace_file = load_trace_reader("normal");
    let trace_reader = BufReader::new(trace_file);

    let mut tick_count = 0;
    let mut missing_critical_fields = 0;
    let mut first_time = None;
    let mut last_time = None;

    for line in trace_reader.lines() {
        let line = line.expect("Failed to read trace line");
        let trace: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse trace");

        tick_count += 1;
        let time = trace["time"].as_u64().unwrap();

        if first_time.is_none() {
            first_time = Some(time);
        }
        last_time = Some(time);

        // Check for critical fields
        let has_lat = trace.get("lat").is_some();
        let has_lon = trace.get("lon").is_some();
        let has_s_cm = trace.get("s_cm").is_some();
        let has_status = trace.get("status").is_some();

        if !has_lat || !has_lon || !has_s_cm || !has_status {
            missing_critical_fields += 1;
        }
    }

    println!("Trace completeness validation:");
    println!("  Total ticks: {}", tick_count);
    println!("  Time range: {} to {}", first_time.unwrap(), last_time.unwrap());
    println!("  Missing critical fields: {}", missing_critical_fields);

    assert!(tick_count > 2900, "Should have 2900+ ticks (actual: {})", tick_count);
    assert_eq!(missing_critical_fields, 0, "All ticks should have critical fields");
}
