# Arrival Detector BDD Test Plan

This document outlines the behavior-driven development (BDD) test scenarios for the `arrival_detector` crate, covering arrival detection logic, state machine transitions, and edge cases.

## Test Status Legend
- ✅ **IMPLEMENTED** - Test exists and passes
- ❌ **MISSING** - Test not yet implemented
- 🔄 **PARTIAL** - Partially covered (test exists but doesn't fully verify scenario)

## 1. Stop Corridor Filtering

### Scenario: Filtering active stops by route progress
*   **Given** a route has three stops at 1km, 5km, and 10km
*   **And** each stop has a corridor of [-80m, +40m]
*   **When** the bus's route progress is at 1010m (within the 1st stop's corridor)
*   **Then** only the 1st stop should be identified as "active"
*   **And** the other stops should be ignored
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival.rs::scenario_close_stop_discrimination`

### Scenario: Handling overlapping corridors
*   **Given** two stops are only 50m apart
*   **And** their corridors overlap (Stop A: [920m, 1040m], Stop B: [970m, 1090m])
*   **When** the bus is at 1000m
*   **Then** both Stop A and Stop B should be identified as "active"
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_edge_cases.rs::scenario_simultaneous_overlapping_corridors`

## 2. Bayesian Arrival Probability

### Scenario: High probability at a full stop
*   **Given** the bus is 5 meters from the stop (Progress Diff ≈ 0)
*   **And** the bus speed is 0 cm/s
*   **And** the bus has been stationary for 15 seconds (Dwell Time > 10s)
*   **When** the arrival probability is calculated
*   **Then** the probability should be very high (exceeding the 75% threshold)
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival.rs::scenario_successful_arrival_at_standard_stop`

### Scenario: Low probability for high-speed pass-by (Skip-Stop Protection)
*   **Given** the bus is 2 meters from the stop
*   **But** the bus speed is 40 km/h (1111 cm/s)
*   **And** the dwell time is only 1 second
*   **When** the arrival probability is calculated
*   **Then** the probability should be low (below the 75% threshold)
*   **And** no arrival should be triggered
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival.rs::scenario_skip_stop_protection_high_speed`

### Scenario: Low probability when far from stop
*   **Given** the bus is stationary (0 cm/s)
*   **But** the bus is 70 meters away from the stop (near the corridor boundary)
*   **When** the arrival probability is calculated
*   **Then** the distance-based likelihood should significantly reduce the total probability
*   **And** remain below the 75% threshold
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_extended.rs::scenario_stationary_but_far_from_stop`

## 3. Stop State Machine (FSM)

### Scenario: Standard arrival sequence
*   **Given** a stop is in the `Approaching` state
*   **When** the bus gets closer than 50m to the stop
*   **Then** the state should transition to `Arriving`
*   **When** the arrival probability then exceeds 75%
*   **Then** the state should transition to `AtStop`
*   **And** an `ArrivalEvent` should be emitted exactly once
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival.rs::scenario_successful_arrival_at_standard_stop`

### Scenario: Premature departure (False Arrival Prevention)
*   **Given** a stop is in the `Arriving` state (within 50m)
*   **When** the bus moves past the stop (d > 40m AND s > stop_progress) without the probability ever exceeding 75%
*   **Then** the state should transition directly to `Departed`
*   **And** no arrival should be recorded
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_extended.rs::scenario_premature_departure`

### Scenario: Stop re-activation (U-Turn / Loop handling)
*   **Given** a stop is in the `Departed` state
*   **When** the bus moves back into the stop's corridor (e.g., after a loop or U-turn)
*   **Then** the state machine should reset back to `Approaching`
*   **And** be ready to detect a second arrival at the same stop
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_extended.rs::scenario_stop_reactivation`

## 4. GPS Jump & Recovery

### Scenario: Forward jump to next stop
*   **Given** the last detected stop was Index 0
*   **When** a GPS jump of 500m occurs, placing the bus near Stop Index 2
*   **Then** the recovery logic should identify Stop Index 2 as the new current stop
*   **And** intervening stops (Index 1) should be marked as skipped or reset
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival.rs::scenario_gps_jump_recovery`

### Scenario: Backward jump penalty (Stability)
*   **Given** the bus is near Stop Index 5
*   **When** GPS noise causes a small backward jump toward Stop Index 4
*   **Then** the index penalty in the recovery scoring should prevent "jumping back" unless the backward movement is very large and persistent
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival.rs::scenario_gps_jump_recovery` includes backward jump test with penalty

## 5. Input & Trace Verification

### Scenario: Heading preservation from localization
*   **Given** a Phase 2 input record with a heading of -950 (350.5°)
*   **When** the record is parsed by the arrival detector
*   **Then** the heading value should be preserved in the `InputRecord`
*   **And** appear correctly in the debug trace output
*   *Status: ✅ IMPLEMENTED* - `test_heading_preservation.rs::test_input_parser_preserves_heading`

### Scenario: Detailed feature trace
*   **Given** the bus is at a stop
*   **When** a trace record is emitted
*   **Then** it should contain individual scores for all 4 features (Distance, Speed, Progress, Dwell Time)
*   **And** the FSM state should be correctly serialized as a string (e.g., "AtStop")
*   *Status: ✅ IMPLEMENTED* - `trace_output.rs` tests trace output format

## 6. Additional Missing Scenarios (Added 2026-03-19)

### Scenario: Simultaneous Overlapping Corridors
*   **Given** two stops have overlapping corridors (Stop A: [920m, 1040m], Stop B: [970m, 1090m])
*   **When** the bus is at 1000m (in the overlap region)
*   **Then** both stops should be returned as active
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_edge_cases.rs::scenario_simultaneous_overlapping_corridors`

### Scenario: Corridor Boundary - Exact Start
*   **Given** a stop with corridor_start_cm = 2000 and corridor_end_cm = 14000
*   **When** the bus progress is exactly 2000
*   **Then** the stop should be identified as active
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_edge_cases.rs::scenario_corridor_boundary_exact_start_and_end`

### Scenario: Corridor Boundary - Exact End
*   **Given** a stop with corridor_start_cm = 2000 and corridor_end_cm = 14000
*   **When** the bus progress is exactly 14000
*   **Then** the stop should be identified as active
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_edge_cases.rs::scenario_corridor_boundary_exact_start_and_end`

### Scenario: Dwell Time Progression
*   **Given** a bus is stationary at a stop (speed = 0 cm/s)
*   **When** multiple updates occur over 5 seconds
*   **Then** the dwell_time_s should increment with each update
*   **And** the accumulated dwell time should increase the arrival probability
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_edge_cases.rs::scenario_dwell_time_progression`

### Scenario: Probability Threshold Edge Case
*   **Given** the arrival probability equals exactly THETA_ARRIVAL (75%)
*   **When** the state update is processed
*   **Then** arrival should NOT be triggered (must be > threshold)
*   *Status: ✅ IMPLEMENTED* - `bdd_arrival_edge_cases.rs::scenario_probability_threshold_edge_case`

### Scenario: GPS Jump Over Entire Corridor
*   **Given** a stop with corridor [2000, 14000]
*   **When** GPS jumps from 1000 to 15000 (skipping the corridor entirely)
*   **Then** the stop should never be marked as active
*   **And** the recovery logic should handle the gap correctly
*   **Status: ✅ IMPLEMENTED** - `bdd_arrival_edge_cases.rs::scenario_gps_jump_over_entire_corridor`

### Scenario: Exit Corridor Without Arrival
*   **Given** a stop is in the `Arriving` state
*   **When** the bus exits the corridor (s_cm > corridor_end_cm) without probability exceeding threshold
*   **Then** the state should transition to `Departed`
*   **And** no arrival event should be emitted
*   *Status: 🔄 PARTIAL* - `scenario_premature_departure` covers this but could be more explicit about corridor exit

### Scenario: Dense Stops - Adjacent Corridors
*   **Given** multiple stops with corridors that touch but don't overlap
*   **When** the bus progresses through the route
*   **Then** only one stop should be active at any given time
*   **And** there should be no gaps in coverage
*   **Status: ✅ IMPLEMENTED** - `bdd_arrival_edge_cases.rs::scenario_dense_stops_adjacent_corridors`
