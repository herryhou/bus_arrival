# Firmware Tests

**Note:** As of 2026-04-06, the firmware tests cannot run due to a pre-existing serde compilation issue in the no_std environment. The tests below are documented for future implementation once the serde issue is resolved.

## Test Cases for Stop Overflow Handling

### Test: `test_stop_count_overflow`
**Purpose:** Verify that routes with >256 stops are handled gracefully

**Setup:**
- Create a RouteData with 300 stops
- Initialize State with this route data

**Expected Behavior:**
1. RouteData should report all 300 stops via `stop_count()`
2. State initialization should:
   - Create exactly 256 StopState entries (heapless::Vec capacity)
   - Log a warning via `defmt::warn!`
   - Not panic or crash
3. Processing GPS updates for stops 0-255 should work normally
4. Stops 256-299 should be silently ignored (no panic)

### Test: `test_stop_count_at_limit`
**Purpose:** Verify that routes with exactly 256 stops work correctly

**Setup:**
- Create a RouteData with 256 stops
- Initialize State with this route data

**Expected Behavior:**
1. All 256 StopState entries should be created
2. No warning should be logged
3. All stops should be processable

### Test: `test_stop_count_normal`
**Purpose:** Verify that routes with <256 stops work correctly

**Setup:**
- Create a RouteData with 50 stops
- Initialize State with this route data

**Expected Behavior:**
1. All 50 StopState entries should be created
2. No warning should be logged
3. All stops should be processable

## Test Cases for DR Speed Decay

### Test: `test_dr_speed_decay_single_second`
**Purpose:** Verify that DR returns decayed speed (dr.filtered_v) not stale Kalman speed

**Setup:**
- Initialize state with v_cms = 1000, dr.filtered_v = 1000
- Simulate GPS outage for 1 second

**Expected Behavior:**
- ProcessResult::DrOutage should return v_cms = 900 (1000 * 9/10)
- NOT v_cms = 1000 (stale Kalman speed)

### Test: `test_dr_speed_decay_multiple_seconds`
**Purpose:** Verify speed decays correctly over multiple seconds

**Setup:**
- Initialize state with v_cms = 1000, dr.filtered_v = 1000
- Simulate GPS outage for 3 seconds

**Expected Behavior:**
- Second 1: v_cms = 900 (1000 * 9/10)
- Second 2: v_cms = 810 (900 * 9/10)
- Second 3: v_cms = 729 (810 * 9/10)

## Implementation Notes

Once the serde issue is resolved, implement these tests in:
- `crates/pico2-firmware/tests/test_stop_overflow.rs` for stop overflow tests
- `crates/pipeline/gps_processor/tests/bdd_localization.rs` for DR speed decay tests (already partially implemented)

The DR speed decay test has already been updated in `bdd_localization.rs`. Run with:
```bash
cargo test --package gps_processor scenario_handle_gps_outage_with_dr
```
