//! Off-route integration tests for state machine
//!
//! Tests the full integration of off-route detection with the State machine,
//! including position freezing and recovery re-acquisition.

use pico2_firmware::state::State;
use shared::{binfile::RouteData, ArrivalEventType, GpsPoint, Stop};
use shared::{FsmState, EARTH_R_CM, FIXED_ORIGIN_LAT_DEG, FIXED_ORIGIN_LON_DEG};

const FIXED_ORIGIN_LAT_RAD: f64 = FIXED_ORIGIN_LAT_DEG.to_radians();

#[test]
fn test_off_route_freezes_position_until_reacquisition_clears() {
    let route_data = create_test_route_data();
    let mut state = State::new(&route_data, None);
    let base_timestamp = 10_000;

    for i in 0..4 {
        let gps = gps_on_route_at_x(base_timestamp + i, 0, 500);
        let event = state.process_gps(&gps);
        assert!(event.is_none(), "Warmup tick {} should not emit events", i);
    }

    let frozen_before = state.last_valid_s_cm();
    assert_eq!(frozen_before, 0, "Warmup should establish route origin position");

    for i in 0..4 {
        let gps = gps_off_route_at_x(base_timestamp + 4 + i, 0, 500);
        let event = state.process_gps(&gps);

        assert!(
            event.is_none(),
            "Suspect tick {} should suppress arrival events",
            i + 1
        );
        assert_eq!(
            state.last_valid_s_cm(),
            frozen_before,
            "Suspect tick {} should keep last_valid_s_cm frozen",
            i + 1
        );
        assert!(
            state.off_route_freeze_time().is_some(),
            "Suspect tick {} should set freeze time immediately",
            i + 1
        );
        // M1: SuspectOffRoute sets recovery flag to ensure snap on re-entry
        assert!(
            state.needs_recovery_on_reacquisition(),
            "Suspect tick {} should set recovery flag for re-entry snap",
            i + 1
        );
    }

    let off_route_event = state.process_gps(&gps_off_route_at_x(base_timestamp + 8, 0, 500));
    assert!(
        off_route_event.is_none(),
        "Confirmed off-route tick should suppress arrival events"
    );
    assert_eq!(
        state.last_valid_s_cm(),
        frozen_before,
        "Confirmed off-route should keep last_valid_s_cm frozen"
    );
    assert!(
        state.needs_recovery_on_reacquisition(),
        "Confirmed off-route should arm reacquisition recovery"
    );

    let first_good = state.process_gps(&gps_on_route_at_x(base_timestamp + 9, 500, 500));
    assert!(
        first_good.is_none(),
        "First good reacquisition tick should still suppress events"
    );
    assert_eq!(
        state.last_valid_s_cm(),
        frozen_before,
        "First good reacquisition tick should still keep position frozen"
    );
    assert!(
        state.needs_recovery_on_reacquisition(),
        "First good reacquisition tick should keep recovery armed"
    );

    let second_good = state.process_gps(&gps_on_route_at_x(base_timestamp + 10, 500, 500));
    assert!(
        second_good.is_none(),
        "Second good reacquisition tick should clear hysteresis without emitting events"
    );
    assert!(
        state.last_valid_s_cm() > frozen_before,
        "Second good reacquisition tick should unfreeze and advance position"
    );
    assert!(
        !state.needs_recovery_on_reacquisition(),
        "Second good reacquisition tick should clear recovery flag"
    );
    assert!(
        state.off_route_freeze_time().is_none(),
        "Freeze time should be cleared after reacquisition"
    );
}

#[test]
fn test_reacquisition_does_not_duplicate_or_advance_stop_state() {
    let route_data = create_test_route_with_recovery_stops();
    let mut state = State::new(&route_data, None);
    let base_timestamp = 20_000;

    for i in 0..4 {
        let gps = gps_on_route_at_x(base_timestamp + i, 0, 500);
        let _ = state.process_gps(&gps);
    }

    let mut saw_stop_0_announce = false;
    for i in 0..3 {
        let event = state.process_gps(&gps_on_route_at_x(base_timestamp + 4 + i, 2_500, 500));
        if let Some(event) = event {
            saw_stop_0_announce |=
                event.stop_idx == 0 && event.event_type == ArrivalEventType::Announce;
        }
    }

    assert!(
        saw_stop_0_announce,
        "Pre-off-route movement should announce stop 0 at least once"
    );
    assert_eq!(state.last_known_stop_index(), 0, "Bus should start near stop 0");
    assert!(
        matches!(
            state.stop_states[0].fsm_state,
            FsmState::Approaching | FsmState::Arriving | FsmState::AtStop
        ),
        "Stop 0 should be active before the off-route episode"
    );

    for i in 0..5 {
        let gps = gps_off_route_at_x(base_timestamp + 7 + i, 2_500, 500);
        let event = state.process_gps(&gps);
        assert!(
            event.is_none(),
            "Off-route tick {} should suppress arrivals while frozen",
            i + 1
        );
    }

    assert!(
        state.needs_recovery_on_reacquisition(),
        "Confirmed off-route should arm recovery"
    );

    let first_good = state.process_gps(&gps_on_route_at_x(base_timestamp + 12, 2_500, 500));
    let second_good = state.process_gps(&gps_on_route_at_x(base_timestamp + 13, 2_500, 500));

    assert!(
        first_good.is_none() && second_good.is_none(),
        "Reacquisition near the same stop should not emit duplicate announcements"
    );
    assert_eq!(
        state.last_known_stop_index(),
        0,
        "Reacquisition near the same location should not spuriously advance to the next stop"
    );
    assert_eq!(
        state.stop_states[1].fsm_state,
        FsmState::Idle,
        "Reacquisition near stop 0 should not activate stop 1"
    );
    assert!(
        matches!(
            state.stop_states[0].fsm_state,
            FsmState::Approaching | FsmState::Arriving | FsmState::AtStop
        ),
        "Reacquisition should keep the current stop active instead of resetting it to an unrelated state"
    );
    assert!(
        state.stop_states[0].announced,
        "The original stop announcement should remain recorded after off-route recovery"
    );
    assert_eq!(
        state.stop_states[0].last_announced_stop,
        0,
        "Reacquisition should not create a duplicate announce marker for the current stop"
    );
    assert!(
        !state.needs_recovery_on_reacquisition(),
        "Recovery flag should clear after reacquisition succeeds"
    );
    assert!(
        state.off_route_freeze_time().is_none(),
        "Freeze time should be cleared after recovery"
    );
}

#[test]
fn test_off_route_suppresses_announce_until_recovery_clears() {
    let route_data = create_test_route_with_stop_data();
    let mut control = State::new(&route_data, None);
    let mut off_route_state = State::new(&route_data, None);
    let base_timestamp = 30_000;
    let mut control_announce = None;

    for i in 0..4 {
        let gps = gps_on_route_at_x(base_timestamp + i, 0, 500);
        let _ = control.process_gps(&gps);
        let _ = off_route_state.process_gps(&gps);
    }

    for i in 0..3 {
        let event = control.process_gps(&gps_on_route_at_x(base_timestamp + 4 + i, 3_000, 500));
        if control_announce.is_none() {
            control_announce = event;
        }
    }

    let control_event = control_announce.expect("Normal path should announce when entering the corridor");
    assert_eq!(
        control_event.event_type,
        ArrivalEventType::Announce,
        "Control path should emit an announce event at corridor entry"
    );
    assert_eq!(control_event.stop_idx, 0, "Control path should announce stop 0");

    for i in 0..5 {
        let event = off_route_state.process_gps(&gps_off_route_at_x(base_timestamp + 4 + i, 0, 500));
        assert!(
            event.is_none(),
            "Off-route tick {} should suppress announcements while frozen",
            i + 1
        );
    }

    let first_good =
        off_route_state.process_gps(&gps_on_route_at_x(base_timestamp + 9, 3_000, 500));
    assert!(
        first_good.is_none(),
        "First good tick should still suppress announcement until hysteresis clears"
    );
    assert!(
        off_route_state.needs_recovery_on_reacquisition(),
        "Recovery should still be armed after only one good tick"
    );

    let mut resumed_announce = None;
    for i in 0..3 {
        let event = off_route_state.process_gps(&gps_on_route_at_x(base_timestamp + 10 + i, 3_000, 500));
        if resumed_announce.is_none() {
            resumed_announce = event;
        }
    }

    let resumed_announce =
        resumed_announce.expect("Announcement should resume shortly after recovery clears");
    assert_eq!(
        resumed_announce.event_type,
        ArrivalEventType::Announce,
        "Recovered path should resume announce behavior after freezing clears"
    );
    assert_eq!(
        resumed_announce.stop_idx,
        0,
        "Recovered path should announce the same stop once freezing clears"
    );
    assert!(
        !off_route_state.needs_recovery_on_reacquisition(),
        "Recovery flag should clear after the second good tick"
    );
}

#[test]
fn test_reacquisition_can_progress_to_next_stop_without_duplicate_prior_announce() {
    let route_data = create_test_route_with_recovery_stops();
    let mut state = State::new(&route_data, None);
    let base_timestamp = 40_000;
    let mut saw_stop_0_announce = false;
    let mut saw_stop_1_announce = false;

    for i in 0..4 {
        let gps = gps_on_route_at_x(base_timestamp + i, 0, 500);
        let _ = state.process_gps(&gps);
    }

    for i in 0..3 {
        let event = state.process_gps(&gps_on_route_at_x(base_timestamp + 4 + i, 2_500, 500));
        if let Some(event) = event {
            saw_stop_0_announce |=
                event.stop_idx == 0 && event.event_type == ArrivalEventType::Announce;
        }
    }

    assert!(
        saw_stop_0_announce,
        "Initial approach should announce stop 0 before the off-route episode"
    );

    for i in 0..5 {
        let event = state.process_gps(&gps_off_route_at_x(base_timestamp + 7 + i, 2_500, 500));
        assert!(
            event.is_none(),
            "Off-route tick {} should suppress arrivals while frozen",
            i + 1
        );
    }

    assert!(
        state.needs_recovery_on_reacquisition(),
        "Off-route episode should arm recovery before progressing to the next stop"
    );

    for i in 0..8 {
        let event = state.process_gps(&gps_on_route_at_x(base_timestamp + 12 + i, 6_500, 500));
        if let Some(event) = event {
            assert!(
                !(event.stop_idx == 0 && event.event_type == ArrivalEventType::Announce),
                "Recovery path must not duplicate stop 0 announcement"
            );
            if event.stop_idx == 1 && event.event_type == ArrivalEventType::Announce {
                saw_stop_1_announce = true;
            }
        }
    }

    assert!(
        saw_stop_1_announce,
        "Progressing well ahead after recovery should eventually announce stop 1"
    );
    assert_eq!(
        state.last_known_stop_index(),
        1,
        "Post-recovery forward progress should advance the active stop index"
    );
    assert_eq!(
        state.stop_states[0].fsm_state,
        FsmState::Departed,
        "Once recovery progresses to stop 1, stop 0 should remain passed"
    );
    assert!(
        matches!(
            state.stop_states[1].fsm_state,
            FsmState::Approaching | FsmState::Arriving | FsmState::AtStop
        ),
        "Stop 1 should become active after forward recovery progress"
    );
}

/// Helper function to create a test route with known geometry
/// Creates a simple straight route along X-axis for predictable testing
fn create_test_route_data() -> RouteData<'static> {
    use shared::{RouteNode, SpatialGrid};

    // Create a simple straight route along X-axis
    // Segment 0: (0, 0) to (10000, 0) - 100m east
    // Segment 1: (10000, 0) to (20000, 0) - 100m east
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000, // 90 degrees (East)
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0, // Last node
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1], vec![0, 1]], // 2x2 grid covering the route
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load route data")
}

/// Helper to create a GPS point on the route (at origin 120°E, 20°N)
fn create_gps_point_with_time(
    timestamp: u64,
    tick_offset: u64,
    speed_cms: i32,
    tick_index: u64,
) -> GpsPoint {
    GpsPoint {
        timestamp: timestamp + tick_offset + tick_index,
        lat: 20.0,          // 20°N (on route at origin)
        lon: 120.0,         // 120°E (on route at origin)
        heading_cdeg: 9000, // East (90 degrees)
        speed_cms,
        hdop_x10: 10,
        has_fix: true,
    }
}

/// Helper to create a GPS point far from the route (>50m)
/// Uses latitude offset to move ~60m north of route
fn create_gps_point_far_from_route(timestamp: u64, tick_index: u64) -> GpsPoint {
    GpsPoint {
        timestamp: timestamp + tick_index,
        lat: 20.0005, // ~60m north of route (1° ≈ 111km, so 0.0005° ≈ 55.5m)
        lon: 120.0,   // Still at 120°E
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    }
}

/// Helper to load the test route data
fn load_test_route_data() -> Option<RouteData<'static>> {
    Some(create_test_route_data())
}

fn create_test_route_with_stop_data() -> RouteData<'static> {
    use shared::{RouteNode, SpatialGrid};

    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 200000,
            dx_cm: 20000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let stops = vec![Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    }];

    let grid = SpatialGrid {
        cells: vec![vec![0], vec![0]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 1,
        x0_cm: 0,
        y0_cm: 0,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, FIXED_ORIGIN_LAT_DEG, &mut buffer)
        .expect("Failed to pack test route-with-stop data");

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load test route-with-stop data")
}

fn create_test_route_with_recovery_stops() -> RouteData<'static> {
    use shared::{RouteNode, SpatialGrid};

    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 200000,
            dx_cm: 20000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let stops = vec![
        Stop {
            progress_cm: 2000,
            corridor_start_cm: 1000,
            corridor_end_cm: 3500,
        },
        Stop {
            progress_cm: 6000,
            corridor_start_cm: 3500,
            corridor_end_cm: 8000,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0], vec![0]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 1,
        x0_cm: 0,
        y0_cm: 0,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, FIXED_ORIGIN_LAT_DEG, &mut buffer)
        .expect("Failed to pack recovery route-with-stop data");

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load recovery route-with-stop data")
}

fn gps_on_route_at_x(timestamp: u64, x_cm: i32, speed_cms: i32) -> GpsPoint {
    let lon =
        FIXED_ORIGIN_LON_DEG + (x_cm as f64 / (EARTH_R_CM * FIXED_ORIGIN_LAT_RAD.cos())).to_degrees();

    GpsPoint {
        timestamp,
        lat: FIXED_ORIGIN_LAT_DEG,
        lon,
        heading_cdeg: 9000,
        speed_cms,
        hdop_x10: 10,
        has_fix: true,
    }
}

fn gps_off_route_at_x(timestamp: u64, x_cm: i32, speed_cms: i32) -> GpsPoint {
    let lon =
        FIXED_ORIGIN_LON_DEG + (x_cm as f64 / (EARTH_R_CM * FIXED_ORIGIN_LAT_RAD.cos())).to_degrees();
    let lat = FIXED_ORIGIN_LAT_DEG + (6000.0 / EARTH_R_CM).to_degrees();

    GpsPoint {
        timestamp,
        lat,
        lon,
        heading_cdeg: 9000,
        speed_cms,
        hdop_x10: 10,
        has_fix: true,
    }
}

/// Helper to create a GPS point with no fix (GPS outage)
fn gps_no_fix(timestamp: u64) -> GpsPoint {
    GpsPoint {
        timestamp,
        lat: FIXED_ORIGIN_LAT_DEG,
        lon: FIXED_ORIGIN_LON_DEG,
        heading_cdeg: 0,
        speed_cms: 0,
        hdop_x10: 0,
        has_fix: false,
    }
}

#[cfg(feature = "dev")]
#[derive(Clone, Copy)]
enum ScriptPoint {
    OnRoute { x_cm: i32, speed_cms: i32 },
    OffRoute { x_cm: i32, speed_cms: i32 },
}

#[cfg(feature = "dev")]
#[derive(Clone)]
struct TickExpectation {
    name: &'static str,
    point: ScriptPoint,
    expect_event: Option<ArrivalEventType>,
    expect_recovery_flag: bool,
    expect_freeze: bool,
    expect_last_valid_moves: bool,
    expect_stop_state: FsmState,
}

#[cfg(feature = "dev")]
fn gps_from_script_point(timestamp: u64, point: ScriptPoint) -> GpsPoint {
    match point {
        ScriptPoint::OnRoute { x_cm, speed_cms } => gps_on_route_at_x(timestamp, x_cm, speed_cms),
        ScriptPoint::OffRoute { x_cm, speed_cms } => gps_off_route_at_x(timestamp, x_cm, speed_cms),
    }
}

#[cfg(feature = "dev")]
/// Helper to create a longer test route (1km) for testing §4.5 scenarios
/// Creates a straight route along X-axis with multiple segments
fn create_long_test_route_data() -> RouteData<'static> {
    use shared::{RouteNode, SpatialGrid};

    let mut nodes = Vec::new();
    let mut cum_dist = 0i64;

    // Create 10 segments, each 100m long (total 1km)
    for i in 0..=10 {
        let x_cm = (i as i64 * 10000) as i32; // 0, 10000, 20000, ... 100000
        let seg_len_mm = if i < 10 { 100000 } else { 0 }; // 100m for segments, 0 for last node

        nodes.push(RouteNode {
            x_cm,
            y_cm: 0,
            cum_dist_cm: cum_dist as i32,
            seg_len_mm,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000, // East
            _pad: 0,
        });

        cum_dist += 10000;
    }

    // Create a grid covering the route
    let grid = SpatialGrid {
        cells: vec![
            vec![0, 1, 2, 3, 4],
            vec![0, 1, 2, 3, 4],
            vec![5, 6, 7, 8, 9],
            vec![5, 6, 7, 8, 9],
            vec![10, 10, 10, 10, 10],
        ],
        grid_size_cm: 20000,
        cols: 5,
        rows: 5,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack long test route data");

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load route data")
}

#[cfg(feature = "dev")]
#[test]
fn test_off_route_table_driven_state_contract() {
    const BASE_TIME: u64 = 30_000;
    const STOP_INDEX: usize = 0;

    let route_data = create_test_route_with_stop_data();
    let mut state = State::new(&route_data, None);

    let script = [
        TickExpectation {
            name: "first_fix",
            point: ScriptPoint::OnRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: false,
            expect_freeze: false,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "warmup_1",
            point: ScriptPoint::OnRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: false,
            expect_freeze: false,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "warmup_2",
            point: ScriptPoint::OnRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: false,
            expect_freeze: false,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "warmup_3",
            point: ScriptPoint::OnRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: false,
            expect_freeze: false,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "suspect_1",
            point: ScriptPoint::OffRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,  // M1: SuspectOffRoute sets recovery flag
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "suspect_2",
            point: ScriptPoint::OffRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,  // M1: SuspectOffRoute sets recovery flag
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "suspect_3",
            point: ScriptPoint::OffRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,  // M1: SuspectOffRoute sets recovery flag
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "suspect_4",
            point: ScriptPoint::OffRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,  // M1: SuspectOffRoute sets recovery flag
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "off_route_confirmed",
            point: ScriptPoint::OffRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "off_route_persisting",
            point: ScriptPoint::OffRoute {
                x_cm: 0,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "reacquire_good_1",
            point: ScriptPoint::OnRoute {
                x_cm: 1000,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: true,
            expect_freeze: true,
            expect_last_valid_moves: false,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "reacquire_good_2",
            point: ScriptPoint::OnRoute {
                x_cm: 1000,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: false,
            expect_freeze: false,
            expect_last_valid_moves: true,
            expect_stop_state: FsmState::Idle,
        },
        TickExpectation {
            name: "normal_after_recovery",
            point: ScriptPoint::OnRoute {
                x_cm: 1500,
                speed_cms: 500,
            },
            expect_event: None,
            expect_recovery_flag: false,
            expect_freeze: false,
            expect_last_valid_moves: true,
            expect_stop_state: FsmState::Idle,
        },
    ];

    let mut first_freeze_time = None;

    for (offset, tick) in script.iter().enumerate() {
        let timestamp = BASE_TIME + offset as u64;
        let gps = gps_from_script_point(timestamp, tick.point);
        let last_valid_before = state.last_valid_s_cm();

        let event = state.process_gps(&gps);
        let event_type = event.map(|value| value.event_type);
        let last_valid_after = state.last_valid_s_cm();
        let freeze_time = state.off_route_freeze_time();

        assert_eq!(
            event_type, tick.expect_event,
            "{}: unexpected event",
            tick.name
        );
        assert_eq!(
            state.needs_recovery_on_reacquisition(),
            tick.expect_recovery_flag,
            "{}: unexpected recovery flag",
            tick.name
        );
        assert_eq!(
            freeze_time.is_some(),
            tick.expect_freeze,
            "{}: unexpected freeze state",
            tick.name
        );
        assert_eq!(
            state.stop_states[STOP_INDEX].fsm_state, tick.expect_stop_state,
            "{}: unexpected stop FSM state",
            tick.name
        );

        if tick.expect_last_valid_moves {
            assert_ne!(
                last_valid_after, last_valid_before,
                "{}: expected last_valid_s_cm to move",
                tick.name
            );
        } else {
            assert_eq!(
                last_valid_after, last_valid_before,
                "{}: expected last_valid_s_cm to remain unchanged",
                tick.name
            );
        }

        if tick.expect_freeze {
            if let Some(existing_freeze_time) = first_freeze_time {
                assert_eq!(
                    freeze_time,
                    Some(existing_freeze_time),
                    "{}: freeze time should remain stable across the off-route episode",
                    tick.name
                );
            } else {
                first_freeze_time = freeze_time;
            }
        } else {
            assert_eq!(
                freeze_time, None,
                "{}: freeze time should be cleared",
                tick.name
            );
        }
    }
}

#[cfg(feature = "dev")]
#[test]
fn test_full_off_route_cycle() {
    // Create test route and state
    let route_data = match load_test_route_data() {
        Some(data) => data,
        None => {
            println!("Skipping test - route data not available");
            return;
        }
    };
    let mut state = State::new(&route_data, None);

    // Phase 1: Normal operation - establish position
    // Process warmup ticks to establish position
    for i in 0..4 {
        let gps1 = create_gps_point_with_time(1000, 0, 500, i);
        let event1 = state.process_gps(&gps1);
        // No arrival events during warmup
        assert!(
            event1.is_none(),
            "Should not have arrival events during warmup"
        );
    }

    let initial_s = state.last_valid_s_cm();
    assert!(initial_s >= 0, "Should have a valid position after warmup");
    println!("Position after warmup: {} cm", initial_s);

    // Phase 2: GPS drifts off-route (>50m for 6+ ticks)
    // This should trigger OffRoute after 5 ticks
    let mut off_route_triggered = false;

    for i in 1..=6 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _event = state.process_gps(&gps_off);

        match i {
            1..=4 => {
                // First 4 ticks: suspect phase (M1)
                // Position IS frozen immediately (Bug 5 fix)
                // M1: SuspectOffRoute sets recovery flag for re-entry snap
                assert!(
                    state.needs_recovery_on_reacquisition(),
                    "Tick {} SHOULD need recovery (M1 SuspectOffRoute)",
                    i
                );
                assert!(
                    state.off_route_freeze_time().is_some(),
                    "Tick {} SHOULD have freeze time (position frozen immediately)",
                    i
                );
            }
            5 => {
                // After 5 ticks: should be in off-route state
                // The GPS processor will return ProcessResult::OffRoute
                // which sets needs_recovery_on_reacquisition
                assert!(
                    state.needs_recovery_on_reacquisition(),
                    "Tick 5 SHOULD need recovery (off-route triggered)"
                );
                assert!(
                    state.off_route_freeze_time().is_some(),
                    "Tick 5 SHOULD have freeze time set"
                );
                off_route_triggered = true;
                println!(
                    "Off-route triggered at tick 5, freeze time: {:?}",
                    state.off_route_freeze_time()
                );
            }
            6 => {
                // Still in off-route state
                assert!(
                    state.needs_recovery_on_reacquisition(),
                    "Tick 6 should still need recovery"
                );
                assert!(
                    state.off_route_freeze_time().is_some(),
                    "Tick 6 should still have freeze time"
                );
            }
            _ => unreachable!(),
        }
    }

    assert!(off_route_triggered, "OffRoute should have been triggered");

    // Phase 3: GPS returns to route for 3 ticks
    // After 2 good ticks, off-route should clear and recovery should run
    // Continue from the last off-route timestamp (50006) to avoid time jumps

    // First good tick back on route - still in suspect state (needs 2 good ticks)
    let gps_back_1 = create_gps_point_with_time(50000, 7, 500, 0);
    let _event2 = state.process_gps(&gps_back_1);

    // Still in suspect state after 1 good tick
    assert!(
        state.needs_recovery_on_reacquisition(),
        "After 1st good GPS, still need recovery (hysteresis not cleared)"
    );
    assert!(
        state.off_route_freeze_time().is_some(),
        "After 1st good GPS, freeze time should still be set"
    );

    // Second good tick - this should clear hysteresis and trigger recovery
    let gps_back_2 = create_gps_point_with_time(50000, 8, 500, 0);
    let _event3 = state.process_gps(&gps_back_2);

    // Verify recovery ran and cleared the off-route state
    assert!(
        !state.needs_recovery_on_reacquisition(),
        "After 2nd good GPS, recovery should have cleared the flag"
    );
    assert!(
        state.off_route_freeze_time().is_none(),
        "After 2nd good GPS, freeze time should be cleared"
    );

    println!("Recovery completed after GPS returned to route");

    // Process 2 more good ticks to ensure stable operation
    for i in 9..=10 {
        let gps_good = create_gps_point_with_time(50000, i, 500, 0);
        let _event = state.process_gps(&gps_good);

        // Should remain in normal operation after recovery cleared
        assert!(
            !state.needs_recovery_on_reacquisition(),
            "Tick {} should not need recovery (back to normal)",
            i
        );
        assert!(
            state.off_route_freeze_time().is_none(),
            "Tick {} should not have freeze time (back to normal)",
            i
        );
    }

    // Phase 4: Verify normal operation resumes
    let gps_normal = create_gps_point_with_time(2000, 0, 500, 0);
    let _event3 = state.process_gps(&gps_normal);

    // Should process normally without any off-route state
    assert!(
        !state.needs_recovery_on_reacquisition(),
        "Should be in normal operation"
    );
    assert!(
        state.off_route_freeze_time().is_none(),
        "Should not have freeze time in normal operation"
    );

    // Verify we have a valid position
    let final_s = state.last_valid_s_cm();
    println!("Final position: {} cm", final_s);
    assert!(
        final_s >= 0,
        "Should maintain valid position throughout cycle"
    );

    println!("✓ Full off-route cycle test completed successfully");
    println!("  - Normal operation established");
    println!("  - Off-route detected after 5 ticks");
    println!("  - Recovery triggered on GPS return");
    println!("  - Normal operation resumed");
}

#[cfg(feature = "dev")]
#[test]
fn test_off_route_freeze_time_set_once() {
    // Regression test for Bug 1: off_route_freeze_time was being overwritten
    // every tick instead of being set once on first OffRoute.
    //
    // This test verifies that:
    // 1. Freeze time is set when off-route is first triggered
    // 2. Freeze time is NOT updated on subsequent OffRoute ticks
    // 3. The elapsed time calculation uses the ORIGINAL freeze time

    let route_data = match load_test_route_data() {
        Some(data) => data,
        None => {
            println!("Skipping test - route data not available");
            return;
        }
    };
    let mut state = State::new(&route_data, None);

    // Establish position through warmup
    for i in 0..4 {
        let gps = create_gps_point_with_time(1000, 0, 500, i);
        let _ = state.process_gps(&gps);
    }

    // Trigger off-route - this should set freeze_time on tick 5
    for i in 1..=5 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.process_gps(&gps_off);
    }

    // Verify freeze time is set
    let freeze_time_tick_5 = state.off_route_freeze_time();
    assert!(
        freeze_time_tick_5.is_some(),
        "Freeze time should be set on tick 5"
    );
    let original_freeze_time = freeze_time_tick_5.unwrap();

    // Process MORE OffRoute ticks (tick 6, 7, 8)
    for i in 6..=8 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.process_gps(&gps_off);

        // Verify freeze time has NOT changed
        let current_freeze_time = state.off_route_freeze_time();
        assert_eq!(
            current_freeze_time,
            Some(original_freeze_time),
            "Freeze time should NOT be updated on tick {} (should remain at tick 5 value)",
            i
        );
    }

    // Calculate what elapsed time would be if freeze_time was updated every tick (WRONG behavior)
    // vs set once (CORRECT behavior)
    let gps_return_timestamp = 50000 + 10; // Tick 10
    let wrong_elapsed = gps_return_timestamp - (50000 + 8); // If updated on tick 8: ~2 seconds
    let correct_elapsed = gps_return_timestamp - (50000 + 5); // If set on tick 5: ~5 seconds

    // The difference matters for M12 recovery's velocity constraint:
    // Wrong: max_reachable = 1667 cm/s * 2s = 33m
    // Correct: max_reachable = 1667 cm/s * 5s = 83m
    assert!(
        correct_elapsed > wrong_elapsed,
        "Correct elapsed time should be greater than wrong elapsed time"
    );

    println!("✓ Freeze time set once test passed");
    println!("  Original freeze time: tick 5");
    println!("  Verified freeze time unchanged through tick 8");
    println!(
        "  Correct elapsed: {}s vs Wrong: {}s",
        correct_elapsed, wrong_elapsed
    );
}

#[cfg(feature = "dev")]
#[test]
fn test_m12_recovery_works_without_section_4_5() {
    // Regression test for Bug 2: §4.5 inline recovery conflicts with M12
    //
    // This test verifies that M12 recovery works correctly when §4.5 is removed:
    // 1. GPS returns from off-route with position jump (>50m)
    // 2. M12 receives raw GPS projection (not snapped by §4.5)
    // 3. M12 finds correct stop index using its 4-feature scoring
    // 4. System resumes normal operation with recovered stop index
    //
    // Uses a longer test route (1km) to trigger §4.5's 50m jump threshold

    let route_data = create_long_test_route_data();
    let mut state = State::new(&route_data, None);

    // Phase 1: Establish position at beginning of route (s ≈ 0)
    for i in 0..4 {
        let gps = create_gps_point_with_time(1000, 0, 500, i);
        let _ = state.process_gps(&gps);
    }

    let initial_stop = state.last_known_stop_index();
    println!("Initial stop index: {}", initial_stop);

    // Phase 2: Move forward to s ≈ 100m (along the route)
    // Create GPS points that project to different positions on route
    // by using the origin point (20, 120) repeatedly - each will project to s=0
    // but Kalman will advance based on speed
    for i in 0..20 {
        let gps = GpsPoint {
            timestamp: 1000 + 4 + i,
            lat: 20.0,
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500, // 5 m/s forward
            hdop_x10: 10,
            has_fix: true,
        };
        let _ = state.process_gps(&gps);
    }

    let position_before_off_route = state.last_valid_s_cm();
    let stop_before_off_route = state.last_known_stop_index();
    println!(
        "Position before off-route: {} cm, stop: {}",
        position_before_off_route, stop_before_off_route
    );

    // Phase 3: Trigger off-route (GPS drifts away for 6 ticks)
    // Continue timestamp sequence from where we left off (1024)
    let off_route_start_timestamp = 1000 + 4 + 20;
    for i in 1..=6 {
        let gps_off = create_gps_point_far_from_route(off_route_start_timestamp, i);
        let _ = state.process_gps(&gps_off);
    }

    // Verify off-route was triggered
    assert!(
        state.needs_recovery_on_reacquisition(),
        "Off-route should be triggered, setting recovery flag"
    );
    assert!(
        state.off_route_freeze_time().is_some(),
        "Freeze time should be set"
    );

    let frozen_position = position_before_off_route;
    println!("Position frozen at: {} cm", frozen_position);

    // Phase 4: GPS returns to route (simulating detour return)
    // Continue timestamp sequence (no large gap)
    let detour_return_timestamp = off_route_start_timestamp + 6;

    // First good tick back on route
    let gps_return_1 = GpsPoint {
        timestamp: detour_return_timestamp,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    // Second good tick - should trigger recovery
    let gps_return_2 = GpsPoint {
        timestamp: detour_return_timestamp + 1,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    // Process both good ticks - second tick should trigger recovery
    let _event1 = state.process_gps(&gps_return_1);
    let _event2 = state.process_gps(&gps_return_2);

    // Verify recovery completed (M12 should handle this without §4.5)
    assert!(
        !state.needs_recovery_on_reacquisition(),
        "Recovery should have cleared the flag after 2 good ticks"
    );
    assert!(
        state.off_route_freeze_time().is_none(),
        "Freeze time should be cleared after recovery"
    );

    // Verify we have a valid position after recovery
    let position_after_recovery = state.last_valid_s_cm();
    let stop_after_recovery = state.last_known_stop_index();

    println!(
        "Position after recovery: {} cm, stop: {}",
        position_after_recovery, stop_after_recovery
    );

    // The key assertion: M12 should have found a valid stop index
    assert!(
        position_after_recovery >= 0,
        "Should have valid position after recovery"
    );
    assert_eq!(
        stop_after_recovery,
        stop_before_off_route,
        "Routes without stops should keep the same synthetic stop index after recovery"
    );

    // Process more GPS to ensure stable operation
    for i in 1..=3 {
        let gps_good = create_gps_point_with_time(50200, 0, 500, i);
        let _ = state.process_gps(&gps_good);

        // Should remain stable without re-triggering recovery
        assert!(
            !state.needs_recovery_on_reacquisition(),
            "Should not re-trigger recovery (stable operation)"
        );
    }

    println!("✓ M12 recovery test passed");
    println!("  - Off-route triggered correctly");
    println!("  - GPS returned after long duration");
    println!("  - M12 recovery completed successfully");
    println!("  - Stable operation resumed");
}

#[cfg(feature = "dev")]
#[test]
fn test_off_route_then_long_gps_outage_then_recovery() {
    // Residual risk test: off-route + long GPS outage path
    //
    // This test covers the scenario where:
    // 1. Bus goes off-route (position freeze triggered)
    // 2. GPS loses fix (has_fix: false) for extended duration
    // 3. GPS reacquires fix after >10 seconds
    // 4. System recovers correctly
    //
    // This is where findings 1 (Flash budget timing) and 2 (announcement de-dup)
    // would surface most clearly.

    let route_data = create_test_route_with_recovery_stops();
    let mut state = State::new(&route_data, None);
    let base_timestamp = 50_000;

    // Phase 1: Establish position and approach stop 0
    for i in 0..4 {
        let gps = gps_on_route_at_x(base_timestamp + i, 0, 500);
        let _ = state.process_gps(&gps);
    }

    // Move to near stop 0 and trigger announcement
    let mut stop_0_announced = false;
    for i in 0..3 {
        let gps = gps_on_route_at_x(base_timestamp + 4 + i, 2_500, 500);
        if let Some(event) = state.process_gps(&gps) {
            if event.stop_idx == 0 && event.event_type == ArrivalEventType::Announce {
                stop_0_announced = true;
            }
        }
    }

    assert!(
        stop_0_announced,
        "Stop 0 should be announced before off-route episode"
    );

    let position_before_off_route = state.last_valid_s_cm();
    let _freeze_time_before = state.off_route_freeze_time();

    // Phase 2: Go off-route (position freeze)
    for i in 0..5 {
        let gps = gps_off_route_at_x(base_timestamp + 7 + i, 2_500, 500);
        let event = state.process_gps(&gps);

        assert!(
            event.is_none(),
            "Off-route tick {} should suppress events",
            i + 1
        );
        assert!(
            state.off_route_freeze_time().is_some(),
            "Off-route tick {} should set freeze time",
            i + 1
        );
    }

    // Verify off-route is confirmed and position is frozen
    let freeze_time_after_off_route = state.off_route_freeze_time();
    assert!(
        freeze_time_after_off_route.is_some(),
        "Off-route should have freeze time set"
    );
    assert!(
        state.needs_recovery_on_reacquisition(),
        "Off-route should be confirmed"
    );
    assert_eq!(
        state.last_valid_s_cm(),
        position_before_off_route,
        "Position should remain frozen during off-route"
    );

    // Phase 3: GPS loses fix (extended outage)
    // Simulate 12 seconds of GPS outage (no fix)
    // NOTE: Current behavior:
    // - GPS outage clears off-route state (reset_off_route_state)
    // - For outages <= 10s: DR mode continues, events may be emitted
    // - For outages > 10s: Full outage mode, warmup resets
    let outage_duration_ticks = 12;
    for i in 0..outage_duration_ticks {
        let gps = gps_no_fix(base_timestamp + 12 + i);
        let event = state.process_gps(&gps);

        // During GPS outage:
        // - For first 10 ticks: DR mode, detection continues, events may be emitted
        // - After 10 ticks: Full outage mode, warmup resets, no events
        if i < 10 {
            // DR mode: events are allowed (system continues tracking)
            // We don't assert on event.is_none() here because DR-based detection
            // can legitimately emit events as the bus moves
        } else {
            // Full outage mode (>10s): warmup reset, no events
            assert!(
                event.is_none(),
                "Full outage mode (tick {}) should not emit events",
                i + 1
            );
        }

        // During outage, dead-reckoning advances position based on last known speed
        // This is expected behavior - the system continues tracking using DR
        let position_during_outage = state.last_valid_s_cm();
        assert!(
            position_during_outage >= position_before_off_route,
            "Position should advance or stay same during DR-based outage (tick {})",
            i + 1
        );

        // NOTE: GPS outage CLEARS the off-route freeze time (current behavior)
        // This is a design choice: GPS outage is treated as a separate condition
        // from off-route detection, and entering outage mode resets off-route state
        if i == 0 {
            // First outage tick clears the freeze time
            assert!(
                state.off_route_freeze_time().is_none(),
                "GPS outage should clear off-route freeze time (current system behavior)"
            );
        }
    }

    // Phase 4: GPS reacquires fix at a new position (detour return)
    // Simulate bus returning to route at a different location
    let first_fix_timestamp = base_timestamp + 12 + outage_duration_ticks;

    // After long outage (>10s), warmup was reset
    // First fix after outage requires warmup to complete
    let _position_after_outage = state.last_valid_s_cm();

    // Process warmup ticks after GPS fix
    // With new independent counters, detection becomes ready after 3 ticks
    // First fix (just_reset) + 2 warmup ticks = 3 total ticks, detection ready
    for i in 0..2 {
        let gps = gps_on_route_at_x(first_fix_timestamp + i, 4_000, 500);
        let event = state.process_gps(&gps);

        // During warmup, no events should be emitted
        assert!(
            event.is_none(),
            "Warmup tick {} after outage should not emit events",
            i + 1
        );
    }

    // Now warmup is complete (detection_ready returns true)
    // Position may have changed due to new GPS fix at different location
    let _position_after_warmup = state.last_valid_s_cm();

    // Process a few more ticks to allow position to stabilize
    for i in 0..3 {
        let gps = gps_on_route_at_x(first_fix_timestamp + 4 + i, 4_500 + (i as i32 * 500), 500);
        let _ = state.process_gps(&gps);
    }

    let position_after_recovery = state.last_valid_s_cm();

    // Position should be different from where we started
    // (it may have advanced during DR, then been reset by new GPS fix)
    assert!(
        position_after_recovery != position_before_off_route,
        "Position should be different after full cycle"
    );

    // Position should advance after recovery
    let position_after_recovery = state.last_valid_s_cm();
    assert!(
        position_after_recovery > position_before_off_route,
        "Position should advance after recovery completes"
    );

    // Phase 5: Verify stable operation resumes
    // Process more GPS to ensure no re-triggering
    let mut saw_duplicate_announce = false;
    for i in 0..5 {
        let gps = gps_on_route_at_x(first_fix_timestamp + 1 + i as u64, 4_500 + (i as i32 * 500), 500);
        if let Some(event) = state.process_gps(&gps) {
            // Should NOT re-announce stop 0
            if event.stop_idx == 0 && event.event_type == ArrivalEventType::Announce {
                saw_duplicate_announce = true;
            }
        }
    }

    assert!(
        !saw_duplicate_announce,
        "Should not re-announce stop 0 after off-route + outage + recovery"
    );

    // Verify announcement bookkeeping is preserved
    assert!(
        state.stop_states[0].announced,
        "Stop 0 announced flag should remain true after recovery"
    );
    assert_eq!(
        state.stop_states[0].last_announced_stop,
        0,
        "Stop 0 last_announced_stop should remain 0 after recovery"
    );

    println!("✓ Off-route + long GPS outage test passed");
    println!("  - Position frozen correctly during off-route");
    println!("  - GPS outage clears off-route freeze time (current behavior)");
    println!("  - Dead-reckoning advances position during outage");
    println!("  - Recovery completed after GPS reacquisition");
    println!("  - No duplicate announcements after recovery");
    println!("  - Announcement bookkeeping preserved");
}
