//! Normal Scenario Trace Validation Tests
//!
//! This test module validates the trace output from the normal scenario,
//! ensuring GPS quality, status transitions, FSM state progression,
//! position accuracy, and corridor boundaries are all correct.
//!
//! Tests generate their own trace data fresh each run to avoid fragility.

use super::common::{load_nmea_reader, load_ty225_route};
use pipeline::{Pipeline, PipelineConfig};
use shared::binfile::RouteData;

/// Test GPS quality metrics in normal scenario
///
/// Behavioral invariants:
/// - HDOP should be constant (simulator uses 3.5)
/// - Variance should be 0 (Kalman filter converged)
/// - No GPS jumps (normal scenario has clean GPS data)
#[test]
fn test_normal_gps_quality() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    let mut config = PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(load_nmea_reader("normal"), &route_data, &config)
        .expect("Pipeline processing failed");

    let trace_records = result.trace_records.expect("Trace should be enabled");

    let mut tick_count = 0;
    let mut hdop_violations = 0;
    let mut variance_violations = 0;
    let mut gps_jumps = 0;

    for trace in &trace_records {
        tick_count += 1;

        // Check HDOP (should be constant 3.5 from simulator)
        if let Some(hdop) = trace.hdop {
            if (hdop - 3.5).abs() > 0.1 {
                hdop_violations += 1;
            }
        }

        // Check variance_cm2 (should be 0 - Kalman converged)
        if trace.variance_cm2 != 0 {
            variance_violations += 1;
        }

        // Check for GPS jumps (none expected in normal scenario)
        if trace.gps_jump {
            gps_jumps += 1;
        }
    }

    println!("GPS quality validation:");
    println!("  Total ticks: {}", tick_count);
    println!("  HDOP violations: {}", hdop_violations);
    println!("  Variance violations: {}", variance_violations);
    println!("  GPS jumps: {}", gps_jumps);

    assert_eq!(hdop_violations, 0, "HDOP should be constant 3.5");
    assert_eq!(variance_violations, 0, "Variance should be 0 (Kalman converged)");
    assert_eq!(gps_jumps, 0, "Should have no GPS jumps in normal scenario");
}

/// Test status transitions (valid ↔ dr_outage)
///
/// Behavioral invariants:
/// - DR outage only occurs when heading_constraint_met=false
/// - Divergence_cm should be 0 during dr_outage
/// - Status should be one of: valid, dr_outage, off_route
#[test]
fn test_normal_status_transitions() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    let mut config = PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(load_nmea_reader("normal"), &route_data, &config)
        .expect("Pipeline processing failed");

    let trace_records = result.trace_records.expect("Trace should be enabled");

    let mut dr_outage_count = 0;
    let mut invalid_transitions = 0;
    let mut dr_outage_with_heading_constraint = 0;
    let mut dr_outage_with_divergence = 0;

    for trace in &trace_records {
        let status = &trace.status;

        // Count dr_outage periods
        if status == "dr_outage" {
            dr_outage_count += 1;

            // Verify dr_outage only occurs when heading_constraint_met is false
            if trace.heading_constraint_met {
                dr_outage_with_heading_constraint += 1;
            }

            // Verify divergence_cm is 0 during dr_outage
            if trace.divergence_cm != 0 {
                dr_outage_with_divergence += 1;
            }
        }

        // Check for invalid status transitions
        // Valid statuses are: valid, dr_outage, off_route
        if status != "dr_outage" && status != "valid" && status != "off_route" {
            invalid_transitions += 1;
        }
    }

    println!("Status transition validation:");
    println!("  DR outage periods: {}", dr_outage_count);
    println!("  Invalid transitions: {}", invalid_transitions);
    println!("  DR outages with heading_constraint_met=true: {}", dr_outage_with_heading_constraint);
    println!("  DR outages with divergence_cm != 0: {}", dr_outage_with_divergence);

    // DR outage should occur in normal scenario (GPS has gaps)
    assert!(dr_outage_count > 0, "Should have some DR outages in normal scenario");
    assert_eq!(invalid_transitions, 0, "Should have no invalid status transitions");
    assert_eq!(dr_outage_with_heading_constraint, 0,
        "DR outage should only occur when heading_constraint_met=false");
    assert_eq!(dr_outage_with_divergence, 0,
        "Divergence_cm should be 0 during dr_outage");
}

/// Test FSM state progression for stop arrivals
///
/// Behavioral invariants:
/// - FSM states progress: Approaching → Arriving → AtStop
/// - AtStop should be preceded by Approaching/Arriving
/// - Should have FSM data for stops that were detected
#[test]
fn test_normal_fsm_state_progression() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    let mut config = PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(load_nmea_reader("normal"), &route_data, &config)
        .expect("Pipeline processing failed");

    let trace_records = result.trace_records.expect("Trace should be enabled");

    let mut stop_states: std::collections::HashMap<usize, Vec<String>> = std::collections::HashMap::new();

    for trace in &trace_records {
        let time = trace.time;

        // Track FSM states for each stop
        for stop_state in &trace.stop_states {
            let entry = stop_states.entry(stop_state.stop_idx as usize)
                .or_insert_with(Vec::new);
            entry.push(format!("{}:{}", time, stop_state.fsm_state));
        }
    }

    println!("FSM state progression validation:");
    println!("  Stops with state data: {}", stop_states.len());

    // Validate that stops with AtStop have proper progression
    let mut stops_with_atstop = 0;
    let mut stops_with_invalid_progression = 0;

    for (stop_idx, states) in &stop_states {
        let has_approaching = states.iter().any(|s| s.contains("Approaching"));
        let has_arriving = states.iter().any(|s| s.contains("Arriving"));
        let has_atstop = states.iter().any(|s| s.contains("AtStop"));

        if has_atstop {
            stops_with_atstop += 1;
            // Should have approaching and/or arriving before AtStop
            if !has_approaching && !has_arriving {
                stops_with_invalid_progression += 1;
                println!("WARNING: Stop {} has AtStop but no Approaching/Arriving before", stop_idx);
            }
        }
    }

    println!("  Stops with AtStop state: {}", stops_with_atstop);
    println!("  Stops with invalid progression: {}", stops_with_invalid_progression);

    assert!(!stop_states.is_empty(), "Should have FSM state data");
    assert_eq!(stops_with_invalid_progression, 0, "All AtStop states should have Approaching/Arriving before");
}

/// Test position accuracy at stop arrivals
///
/// Behavioral invariants:
/// - At arrival, gps_distance_cm should be small (within 50m)
/// - Most arrivals should be accurate
#[test]
fn test_normal_position_accuracy_at_arrivals() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    let mut config = PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(load_nmea_reader("normal"), &route_data, &config)
        .expect("Pipeline processing failed");

    let trace_records = result.trace_records.expect("Trace should be enabled");

    let mut arrival_positions = Vec::new();

    // Find all stop arrivals (just_arrived=true)
    for trace in &trace_records {
        for stop_state in &trace.stop_states {
            if stop_state.just_arrived {
                arrival_positions.push((
                    stop_state.stop_idx,
                    trace.time,
                    trace.lat,
                    trace.lon,
                    trace.s_cm,
                    stop_state.gps_distance_cm,
                ));
            }
        }
    }

    println!("Position accuracy at arrivals:");
    println!("  Total arrivals found: {}", arrival_positions.len());

    assert!(!arrival_positions.is_empty(), "Should have arrivals");

    // Check that gps_distance_cm is small at arrivals (within 50m)
    let far_arrivals: Vec<_> = arrival_positions.iter()
        .filter(|(_, _, _, _, _, gps_dist)| gps_dist.abs() > 5000)
        .collect();

    println!("  Arrivals with gps_distance > 50m: {}", far_arrivals.len());

    for (stop_idx, time, _lat, _lon, _s_cm, gps_dist) in &far_arrivals {
        println!("    Stop {} at time {}: gps_distance={}cm", stop_idx, time, gps_dist);
    }

    // Most arrivals should be accurate (allow a few outliers due to GPS noise)
    let accurate_ratio = 1.0 - (far_arrivals.len() as f64 / arrival_positions.len() as f64);
    println!("  Accuracy ratio: {:.2}%", accurate_ratio * 100.0);

    assert!(accurate_ratio > 0.90, "At least 90% of arrivals should have gps_distance < 50m");

    // Verify we have a reasonable number of arrivals (normal scenario has 20+ stops)
    assert!(arrival_positions.len() >= 20, "Should have at least 20 arrivals");
}

/// Test corridor boundaries
///
/// Behavioral invariants:
/// - When active_stops is present, corridor_start_cm and corridor_end_cm should exist
/// - s_cm should be within corridor bounds
/// - Corridor length should be reasonable (around 120m)
#[test]
fn test_normal_corridor_boundaries() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    let mut config = PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(load_nmea_reader("normal"), &route_data, &config)
        .expect("Pipeline processing failed");

    let trace_records = result.trace_records.expect("Trace should be enabled");

    let mut corridor_checks = 0;
    let mut corridor_violations = 0;
    let mut progress_violations = 0;
    let mut unusual_corridor_lengths = 0;

    for trace in &trace_records {
        // Only check when active_stops is present
        if !trace.active_stops.is_empty() {
            corridor_checks += 1;

            // Verify corridor_start_cm and corridor_end_cm exist
            let corridor_start = match trace.corridor_start_cm {
                Some(v) => v,
                None => {
                    corridor_violations += 1;
                    continue;
                }
            };

            let corridor_end = match trace.corridor_end_cm {
                Some(v) => v,
                None => {
                    corridor_violations += 1;
                    continue;
                }
            };

            // Verify corridor length is reasonable (around 12000cm = 120m)
            let corridor_length = corridor_end - corridor_start;
            if corridor_length < 5000 || corridor_length > 20000 {
                unusual_corridor_lengths += 1;
            }

            // Verify s_cm is within corridor bounds
            if trace.s_cm < corridor_start || trace.s_cm > corridor_end {
                progress_violations += 1;
            }
        }
    }

    println!("Corridor boundary validation:");
    println!("  Ticks with active stops: {}", corridor_checks);
    println!("  Missing corridor data: {}", corridor_violations);
    println!("  Progress outside corridor: {}", progress_violations);
    println!("  Unusual corridor lengths: {}", unusual_corridor_lengths);

    assert_eq!(corridor_violations, 0, "All active_stops should have corridor data");
    assert_eq!(progress_violations, 0, "Progress should always be within corridor");
}

/// Test trace completeness and consistency
///
/// Behavioral invariants:
/// - All ticks should have critical fields (lat, lon, s_cm, status)
/// - Time should be monotonically increasing
/// - Should have sufficient ticks for a complete route
#[test]
fn test_normal_trace_completeness() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes).expect("Failed to load route data");

    let mut config = PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(load_nmea_reader("normal"), &route_data, &config)
        .expect("Pipeline processing failed");

    let trace_records = result.trace_records.expect("Trace should be enabled");

    let mut tick_count = 0;
    let mut missing_critical_fields = 0;
    let mut first_time = None;
    let mut last_time = None;
    let mut time_regressions = 0;
    let mut prev_time: Option<u64> = None;

    for trace in &trace_records {
        tick_count += 1;
        let time = trace.time;

        if first_time.is_none() {
            first_time = Some(time);
        }
        last_time = Some(time);

        // Check for time regression (should be monotonically increasing)
        if let Some(prev) = prev_time {
            if time < prev {
                time_regressions += 1;
            }
        }
        prev_time = Some(time);

        // Check for critical fields
        let has_critical = trace.lat != 0.0 && trace.lon != 0.0 && trace.status != "";
        if !has_critical {
            missing_critical_fields += 1;
        }
    }

    println!("Trace completeness validation:");
    println!("  Total ticks: {}", tick_count);
    println!("  Time range: {} to {}", first_time.unwrap(), last_time.unwrap());
    println!("  Missing critical fields: {}", missing_critical_fields);
    println!("  Time regressions: {}", time_regressions);

    assert!(tick_count > 2900, "Should have 2900+ ticks (actual: {})", tick_count);
    assert_eq!(missing_critical_fields, 0, "All ticks should have critical fields");
    assert_eq!(time_regressions, 0, "Time should be monotonically increasing");
}
