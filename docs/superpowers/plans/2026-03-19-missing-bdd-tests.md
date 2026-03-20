# Missing BDD Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-step. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement missing BDD test scenarios for simulator and arrival_detector crates to improve edge case coverage.

**Architecture:** Add new test scenario functions to existing BDD test files, following established GWT (Given-When-Then) pattern. Create route builder helpers for complex route geometries (L-shaped, circular).

**Tech Stack:** Rust, cargo test framework, existing simulator/arrival_detector crates

---

## File Structure

### Files to Modify
- `simulator/tests/bdd_localization.rs` - Add new test scenarios and route builders
- `arrival_detector/tests/bdd_arrival_edge_cases.rs` - Add new test scenarios

### New Route Builders to Create (in `bdd_localization.rs`)
- `setup_l_shaped_route()` - L-shaped route with 90° turn
- `setup_circular_route()` - Circular/loop route where start=end

---

## Task 1: Add L-Shaped Route Builder

**Files:**
- Modify: `simulator/tests/bdd_localization.rs`

**Context:** L-shaped routes test segment transitions through 90° turns. Route has two perpendicular segments (horizontal then vertical).

- [ ] **Step 1: Write the L-shaped route builder function**

Add to `simulator/tests/bdd_localization.rs` after `setup_test_route_data()`:

```rust
// Creates an L-shaped route: 50m East, then 50m North
// Returns (buffer, start_x, start_y)
fn setup_l_shaped_route() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();

    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    const BASE_LAT: f64 = 25.0;
    const BASE_LON: f64 = 121.0;
    let lat_avg_rad = BASE_LAT.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let lon_rad = BASE_LON.to_radians();
    let lat_rad = BASE_LAT.to_radians();
    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let start_x = (x_abs - x0_abs).round() as i32;
    let start_y = (y_abs - y0_abs).round() as i32;

    // Segment 0: 50m East (heading = 9000 cdeg = 90°)
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: 9000,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 0,
        dx_cm: 5000,
        dy_cm: 0,
        seg_len_cm: 5000,
    });

    // Corner node (end of seg 0, start of seg 1)
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: 0,  // North
        _pad: 0,
        x_cm: start_x + 5000,
        y_cm: start_y,
        cum_dist_cm: 5000,
        dx_cm: 0,
        dy_cm: 5000,
        seg_len_cm: 5000,
    });

    // End node
    nodes.push(RouteNode {
        len2_cm2: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x + 5000,
        y_cm: start_y + 5000,
        cum_dist_cm: 10000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
    });

    // Grid covering both segments
    let grid = shared::SpatialGrid {
        cells: vec![vec![0, 1]],
        grid_size_cm: 5000,
        cols: 2,
        rows: 1,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack L-shaped route");

    (buffer, start_x, start_y)
}
```

- [ ] **Step 2: Verify the code compiles**

Run: `cargo test -p simulator --test bdd_localization --no-run`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add simulator/tests/bdd_localization.rs
git commit -m "test(simulator): add L-shaped route builder for BDD tests"
```

---

## Task 2: Implement L-Shaped Turn Test

**Files:**
- Modify: `simulator/tests/bdd_localization.rs`

- [ ] **Step 1: Write the L-shaped turn test scenario**

Add after existing scenario functions:

```rust
fn scenario_l_shaped_turn(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Bus starts at the beginning of L-shaped route (going East)
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = 9000; // East
    gps.speed_cms = 500; // 5 m/s

    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);
    if let ProcessResult::Valid { seg_idx, s_cm, .. } = result {
        assert_eq!(seg_idx, 0, "Should start on segment 0 (East)");
        assert_eq!(s_cm, 0);
    } else {
        panic!("Initial fix failed");
    }

    // When: Bus moves 25m East (halfway through first segment)
    gps.timestamp += 5;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x + 2500, route_data.lat_avg_deg);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    if let ProcessResult::Valid { seg_idx, s_cm, .. } = result {
        assert_eq!(seg_idx, 0, "Should still be on segment 0");
        assert_eq!(s_cm, 2500, "Progress should be 25m");
    } else {
        panic!("Mid-segment update failed");
    }

    // When: Bus reaches the corner and turns North
    gps.timestamp += 5;
    gps.lat = lat_from_y(start_y + 2500); // 25m North from corner
    gps.lon = lon_from_x(start_x + 5000, route_data.lat_avg_deg); // At corner x
    gps.heading_cdeg = 0; // North now
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 2, false);

    // Then: Map matcher should identify segment 1 (North)
    if let ProcessResult::Valid { seg_idx, s_cm, .. } = result {
        assert_eq!(seg_idx, 1, "Should transition to segment 1 (North)");
        assert_eq!(s_cm, 7500, "Progress should be 50m + 25m = 75m");
    } else {
        panic!("Turn transition failed");
    }
}
```

- [ ] **Step 2: Add the test to the main test function**

Modify `test_localization_behavioral_scenarios()` to include. The existing function is:

```rust
#[test]
fn test_localization_behavioral_scenarios() {
    let (buffer, start_x, start_y) = setup_test_route_data();
    let route_data = RouteData::load(&buffer).expect("Failed to load test route data");

    scenario_normal_forward_movement(&route_data, start_x, start_y);
    scenario_handle_gps_jump(&route_data, start_x, start_y);
    scenario_handle_gps_outage_with_dr(&route_data, start_x, start_y);
    scenario_heading_penalty_overlapping_routes(&route_data, start_x, start_y);
    scenario_monotonicity_tolerance(&route_data, start_x, start_y);
    scenario_max_speed_rejection(&route_data, start_x, start_y);
    scenario_hdop_adaptive_smoothing(&route_data, start_x, start_y);
    scenario_extended_gps_outage(&route_data, start_x, start_y);
    scenario_route_end_clamping(&route_data, start_x, start_y);
}
```

Add these lines BEFORE the closing brace:

```rust
    // L-shaped route tests
    let (l_buffer, l_start_x, l_start_y) = setup_l_shaped_route();
    let l_route_data = RouteData::load(&l_buffer).expect("Failed to load L-shaped route");
    scenario_l_shaped_turn(&l_route_data, l_start_x, l_start_y);
}
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p simulator --test bdd_localization`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add simulator/tests/bdd_localization.rs
git commit -m "test(simulator): add L-shaped turn BDD scenario"
```

---

## Task 3: Add Circular Route Builder

**Files:**
- Modify: `simulator/tests/bdd_localization.rs`

- [ ] **Step 1: Write the circular route builder function**

Add after `setup_l_shaped_route()`:

```rust
// Creates a circular/loop route where start and end are at the same location
// Uses a square pattern: East 50m, North 50m, West 50m, South 50m back to start
fn setup_circular_route() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();

    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    const BASE_LAT: f64 = 25.0;
    const BASE_LON: f64 = 121.0;
    let lat_avg_rad = BASE_LAT.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let lon_rad = BASE_LON.to_radians();
    let lat_rad = BASE_LAT.to_radians();
    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let start_x = (x_abs - x0_abs).round() as i32;
    let start_y = (y_abs - y0_abs).round() as i32;

    // Segment 0: East 50m (heading 9000)
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: 9000,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 0,
        dx_cm: 5000,
        dy_cm: 0,
        seg_len_cm: 5000,
    });

    // Node 1: Corner NE
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x + 5000,
        y_cm: start_y,
        cum_dist_cm: 5000,
        dx_cm: 0,
        dy_cm: 5000,
        seg_len_cm: 5000,
    });

    // Node 2: Corner NW
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: -9000, // West
        _pad: 0,
        x_cm: start_x + 5000,
        y_cm: start_y + 5000,
        cum_dist_cm: 10000,
        dx_cm: -5000,
        dy_cm: 0,
        seg_len_cm: 5000,
    });

    // Node 3: Corner SW
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: -18000, // South
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y + 5000,
        cum_dist_cm: 15000,
        dx_cm: 0,
        dy_cm: -5000,
        seg_len_cm: 5000,
    });

    // Node 4: Back to start (end = start coordinates)
    nodes.push(RouteNode {
        len2_cm2: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 20000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
    });

    let grid = shared::SpatialGrid {
        cells: vec![vec![0, 1, 2], vec![3, 0, 1], vec![2, 3, 0]],
        grid_size_cm: 5000,
        cols: 3,
        rows: 3,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack circular route");

    (buffer, start_x, start_y)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo test -p simulator --test bdd_localization --no-run`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add simulator/tests/bdd_localization.rs
git commit -m "test(simulator): add circular/loop route builder for BDD tests"
```

---

## Task 4: Implement Loop Closure Test

**Files:**
- Modify: `simulator/tests/bdd_localization.rs`

- [ ] **Step 1: Write the loop closure test scenario**

```rust
fn scenario_loop_closure(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Bus starts at the beginning of a loop route
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = 9000; // East
    gps.speed_cms = 500;

    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 0, "Should start at progress 0");
    } else {
        panic!("Initial fix failed");
    }

    // When: Bus completes 3/4 of the loop (at corner 3, progress = 15000)
    // This is near the start coordinates but with different progress
    gps.timestamp += 30;
    gps.lat = lat_from_y(start_y + 5000); // North corner
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = -18000; // South
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: Progress should be 15000 (3/4 around), not 0
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert!(s_cm > 10000, "Progress should be > 10000, got {}", s_cm);
        assert!(s_cm < 20000, "Progress should be < 20000, got {}", s_cm);
    } else {
        panic!("Near-start update failed");
    }

    // When: Bus completes the full loop (back at start, progress = 20000)
    gps.timestamp += 10;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = 9000; // East again
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 2, false);

    // Then: Progress should clamp to route length (20000)
    // And should NOT jump back to 0
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 20000, "Progress should be at route end (20000), not 0");
    } else {
        panic!("Loop completion update failed");
    }
}
```

- [ ] **Step 2: Add to main test function**

Add these lines to `test_localization_behavioral_scenarios()` BEFORE the closing brace (after the L-shaped test):

```rust
    // Circular route tests
    let (c_buffer, c_start_x, c_start_y) = setup_circular_route();
    let c_route_data = RouteData::load(&c_buffer).expect("Failed to load circular route");
    scenario_loop_closure(&c_route_data, c_start_x, c_start_y);
}

- [ ] **Step 3: Run test**

Run: `cargo test -p simulator --test bdd_localization`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add simulator/tests/bdd_localization.rs
git commit -m "test(simulator): add loop closure BDD scenario"
```

---

## Task 5: Implement GPS Jump Over Stop Corridor Test

**Files:**
- Modify: `arrival_detector/tests/bdd_arrival_edge_cases.rs`

- [ ] **Step 1: Write the GPS jump over corridor test**

Add to `bdd_arrival_edge_cases.rs`:

```rust
#[test]
fn scenario_gps_jump_over_entire_corridor() {
    use arrival_detector::corridor::find_active_stops;

    // Given: a stop with corridor [2000, 14000]
    let stops = vec![
        Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
    ];

    // When: checking progress before corridor
    let active_before = find_active_stops(1000, &stops);
    assert_eq!(active_before.len(), 0, "Should not be active before corridor");

    // When: GPS jumps to 15000 (skipping the corridor entirely)
    let active_after = find_active_stops(15000, &stops);

    // Then: the stop should never be marked as active
    assert_eq!(active_after.len(), 0, "Should not be active after jumping over corridor");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p arrival_detector --test bdd_arrival_edge_cases`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add arrival_detector/tests/bdd_arrival_edge_cases.rs
git commit -m "test(arrival_detector): add GPS jump over corridor BDD scenario"
```

---

## Task 6: Implement Route Reversal Detection Test

**Files:**
- Modify: `simulator/tests/bdd_localization.rs`

- [ ] **Step 1: Write the route reversal test**

```rust
fn scenario_route_reversal_detection(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Bus is at 50m moving North (heading = 0)
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y + 50000);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = 0; // North
    gps.speed_cms = 1000;

    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 50000);
    } else {
        panic!("Initial position failed");
    }

    // When: GPS suddenly shows movement South (opposite direction)
    // with significant backward progress (50m -> 10m)
    gps.timestamp += 10;
    gps.lat = lat_from_y(start_y + 10000); // Jumped back to 10m
    gps.heading_cdeg = 18000; // South (opposite)
    gps.speed_cms = 1000;
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: Should be rejected due to monotonicity (40000cm backward > 50000cm tolerance)
    // Note: We give enough time (10s) to pass speed constraint
    match result {
        ProcessResult::Rejected(reason) => {
            assert_eq!(reason, "monotonicity", "Backward jump should be rejected by monotonicity");
        }
        ProcessResult::Valid { .. } => {
            panic!("Backward jump of 40000cm should be rejected, but was accepted");
        }
        _ => {}
    }
}
```

- [ ] **Step 2: Add to main test function**

Add this line to `test_localization_behavioral_scenarios()` BEFORE the closing brace:

```rust
    scenario_route_reversal_detection(&route_data, start_x, start_y);
}
```

- [ ] **Step 3: Run test**

Run: `cargo test -p simulator --test bdd_localization`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add simulator/tests/bdd_localization.rs
git commit -m "test(simulator): add route reversal detection BDD scenario"
```

---

## Task 7: Implement Dense Stops - Adjacent Corridors Test

**Files:**
- Modify: `arrival_detector/tests/bdd_arrival_edge_cases.rs`

- [ ] **Step 1: Write the dense stops test**

```rust
#[test]
fn scenario_dense_stops_adjacent_corridors() {
    use arrival_detector::corridor::find_active_stops;

    // Given: multiple stops with corridors that touch but don't overlap
    // Stop 0: [0, 10000]
    // Stop 1: [10000, 20000] - starts where Stop 0 ends
    // Stop 2: [20000, 30000] - starts where Stop 1 ends
    let stops = vec![
        Stop { progress_cm: 5000, corridor_start_cm: 0, corridor_end_cm: 10000 },
        Stop { progress_cm: 15000, corridor_start_cm: 10000, corridor_end_cm: 20000 },
        Stop { progress_cm: 25000, corridor_start_cm: 20000, corridor_end_cm: 30000 },
    ];

    // When: checking at boundary between Stop 0 and Stop 1
    let active_at_boundary = find_active_stops(10000, &stops);

    // Then: both stops should be active (at exact boundary)
    assert_eq!(active_at_boundary.len(), 2, "Both stops should be active at boundary");

    // When: checking in middle of Stop 1's corridor
    let active_middle = find_active_stops(15000, &stops);

    // Then: only Stop 1 should be active
    assert_eq!(active_middle, vec![1], "Only Stop 1 should be active");

    // Verify no gaps: every point should have at least one active stop
    for progress in [0, 5000, 10000, 15000, 20000, 25000, 30000].iter() {
        let active = find_active_stops(*progress, &stops);
        assert!(active.len() >= 1, "Should have at least one active stop at progress {}", progress);
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p arrival_detector --test bdd_arrival_edge_cases`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add arrival_detector/tests/bdd_arrival_edge_cases.rs
git commit -m "test(arrival_detector): add dense stops adjacent corridors BDD scenario"
```

---

## Task 8: Implement GPS Jump Over Stop Corridor Test (Simulator)

**Files:**
- Modify: `simulator/tests/bdd_localization.rs`

**Context:** This tests the simulator's active stops detection when GPS jumps over a corridor. This is separate from arrival_detector's corridor filter test.

- [ ] **Step 1: Create route with a stop for GPS jump test**

Add route builder:

```rust
fn setup_route_with_stop() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();

    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    const BASE_LAT: f64 = 25.0;
    const BASE_LON: f64 = 121.0;
    let lat_avg_rad = BASE_LAT.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let lon_rad = BASE_LON.to_radians();
    let lat_rad = BASE_LAT.to_radians();
    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let start_x = (x_abs - x0_abs).round() as i32;
    let start_y = (y_abs - y0_abs).round() as i32;

    // 200m route
    nodes.push(RouteNode {
        len2_cm2: 20000 * 20000,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 20000,
        seg_len_cm: 20000,
    });

    nodes.push(RouteNode {
        len2_cm2: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y + 20000,
        cum_dist_cm: 20000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
    });

    // Stop at 100m with corridor [80m, 120m]
    let stops = vec![
        shared::Stop {
            progress_cm: 10000,
            corridor_start_cm: 8000,
            corridor_end_cm: 12000,
        }
    ];

    let grid = shared::SpatialGrid {
        cells: vec![vec![0]],
        grid_size_cm: 20000,
        cols: 1,
        rows: 1,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack route with stop");

    (buffer, start_x, start_y)
}
```

- [ ] **Step 2: Write GPS jump over corridor test**

```rust
fn scenario_gps_jump_over_corridor(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Bus is at 70m (before corridor)
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y + 7000);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.speed_cms = 500;

    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);
    if let ProcessResult::Valid { s_cm, active_stops, .. } = result {
        assert_eq!(s_cm, 7000);
        assert!(active_stops.is_empty(), "Should not have active stops before corridor");
    } else {
        panic!("Initial position failed");
    }

    // When: GPS jumps from 70m to 130m (skipping the 80-120m corridor entirely)
    // Give enough time to pass speed constraint
    gps.timestamp += 20; // 20 seconds
    gps.lat = lat_from_y(start_y + 13000);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: No active stops should be detected (corridor was never sampled)
    if let ProcessResult::Valid { active_stops, .. } = result {
        assert!(active_stops.is_empty(), "Should have no active stops after jumping over corridor");
    } else {
        panic!("GPS jump update failed");
    }
}
```

- [ ] **Step 3: Add to main test function**

Add to `test_localization_behavioral_scenarios()` BEFORE the closing brace:

```rust
    // GPS jump over corridor test
    let (s_buffer, s_start_x, s_start_y) = setup_route_with_stop();
    let s_route_data = RouteData::load(&s_buffer).expect("Failed to load route with stop");
    scenario_gps_jump_over_corridor(&s_route_data, s_start_x, s_start_y);
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p simulator --test bdd_localization`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add simulator/tests/bdd_localization.rs
git commit -m "test(simulator): add GPS jump over stop corridor BDD scenario"
```

---

## Task 9: Implement Stop Re-activation After Loop Test

**Files:**
- Modify: `arrival_detector/tests/bdd_arrival_edge_cases.rs`

- [ ] **Step 1: Write stop re-activation after loop test**

```rust
#[test]
fn scenario_stop_reactivation_after_loop() {
    // Given: a stop with corridor [2000, 14000]
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    let mut state = StopState::new(0);

    // When: bus arrives at stop
    state.update(10000, 0, stop.progress_cm, 200); // High probability triggers arrival
    assert_eq!(state.fsm_state, FsmState::AtStop);

    // When: bus departs (moves past stop)
    state.update(15000, 500, stop.progress_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Departed);

    // When: bus loops back and enters corridor again (e.g., circular route)
    let can_reset = state.can_reactivate(5000, stop.progress_cm);
    assert!(can_reset, "Should be able to re-enter corridor after loop");

    state.reset();
    assert_eq!(state.fsm_state, FsmState::Approaching);
    assert_eq!(state.dwell_time_s, 0, "Dwell time should reset");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p arrival_detector --test bdd_arrival_edge_cases`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add arrival_detector/tests/bdd_arrival_edge_cases.rs
git commit -m "test(arrival_detector): add stop re-activation after loop BDD scenario"
```

---

## Task 10: Update BDD Test Plans

**Files:**
- Modify: `simulator/tests/BDD_TEST_PLAN.md`
- Modify: `arrival_detector/tests/BDD_TEST_PLAN.md`

- [ ] **Step 1: Update simulator BDD plan status**

In `simulator/tests/BDD_TEST_PLAN.md`, update these lines:

Line 27: "Handling sharp turns" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_l_shaped_turn`

Line 32: Change `*Status: ❌ MISSING* - Need test with L-shaped or 90° turn route` to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_l_shaped_turn`

Line 43: "Heading-constrained segment selection" → Keep `*Status: 🔄 PARTIAL*` (already correct)

Line 50: "Recovery via Grid Search" → Keep `*Status: ❌ MISSING*` OR change to `*Status: 🔄 PARTIAL* - Covered by scenario_handle_gps_jump`

Line 96: "Route Reversal Detection" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_route_reversal_detection`

Line 114: "Loop Closure" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_loop_closure`

Line 119: "Loop Closure" (duplicate) → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_loop_closure`

Line 142: "Skipping a Stop" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_gps_jump_over_corridor`

Line 158: "L-Shaped Route with 90° Turn" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_l_shaped_turn`

Line 164: "Circular Route (Loop)" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_loop_closure`

Line 177: "Stop Re-activation After Loop" → Change to `*Status: ✅ IMPLEMENTED* - arrival_detector/tests/bdd_arrival_edge_cases.rs::scenario_stop_reactivation_after_loop`

Line 184: "GPS Jump Over Stop Corridor" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_gps_jump_over_corridor`

Line 190: "Opposite Direction Rejection" → Change to `*Status: ✅ IMPLEMENTED* - bdd_localization.rs::scenario_route_reversal_detection`

- [ ] **Step 2: Update arrival_detector BDD plan status**

In `arrival_detector/tests/BDD_TEST_PLAN.md`, update these lines:

Line 25: "Handling overlapping corridors" → Change `*Status: 🔄 PARTIAL*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_simultaneous_overlapping_corridors`

Line 112: "Simultaneous Overlapping Corridors" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_simultaneous_overlapping_corridors`

Line 122: "Corridor Boundary - Exact Start" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_corridor_boundary_exact_start_and_end`

Line 128: "Corridor Boundary - Exact End" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_corridor_boundary_exact_start_and_end`

Line 135: "Dwell Time Progression" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_dwell_time_progression`

Line 141: "Probability Threshold Edge Case" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_probability_threshold_edge_case`

Line 148: "GPS Jump Over Entire Corridor" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_gps_jump_over_entire_corridor`

Line 162: "Dense Stops - Adjacent Corridors" → Change `*Status: ❌ MISSING*` to `*Status: ✅ IMPLEMENTED* - bdd_arrival_edge_cases.rs::scenario_dense_stops_adjacent_corridors`

Note: Lines 110-117 already have `✅ IMPLEMENTED` status for "Simultaneous Overlapping Corridors" - this is a duplicate. Keep the one at line 112 and remove/update the one at line 25.

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p simulator --test bdd_localization && cargo test -p arrival_detector --test bdd_arrival_edge_cases`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add simulator/tests/BDD_TEST_PLAN.md arrival_detector/tests/BDD_TEST_PLAN.md
git commit -m "docs: update BDD test plans with implemented scenarios"
```

---

## Verification Steps

After completing all tasks:

- [ ] **Run all affected tests**

```bash
cargo test -p simulator --test bdd_localization -v
cargo test -p arrival_detector --test bdd_arrival_edge_cases -v
```

Expected: All tests pass

- [ ] **Run full test suite**

```bash
cargo test --workspace
```

Expected: No regressions

- [ ] **Review coverage**

The following scenarios should now be covered:
- ✅ L-shaped route with 90° turn (simulator)
- ✅ Circular/loop route with closure detection (simulator)
- ✅ Route reversal/opposite direction detection (simulator)
- ✅ GPS jump over corridor (simulator + arrival_detector)
- ✅ Dense stops with adjacent corridors (arrival_detector)
- ✅ Stop re-activation after loop (arrival_detector)

---

## Summary

**New Test Functions Added:**
- `setup_l_shaped_route()` - Route builder for L-shaped routes
- `setup_circular_route()` - Route builder for circular/loop routes
- `setup_route_with_stop()` - Route builder with stop for GPS jump tests
- `scenario_l_shaped_turn()` - L-shaped turn test (90° segment transition)
- `scenario_loop_closure()` - Loop closure test (circular route)
- `scenario_route_reversal_detection()` - Route reversal/opposite direction test
- `scenario_gps_jump_over_corridor()` - GPS jump over stop corridor (simulator)
- `scenario_gps_jump_over_entire_corridor()` - GPS jump over corridor (arrival_detector)
- `scenario_dense_stops_adjacent_corridors()` - Dense stops with touching corridors
- `scenario_stop_reactivation_after_loop()` - Stop FSM reset after loop completion

**Estimated Time:** 2-3 hours

**Dependencies:** None (uses existing test infrastructure)
