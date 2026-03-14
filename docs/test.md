As a Senior QA, I have designed a comprehensive BDD-style test plan for the Bus Arrival Detection System (v8.0). 


  This plan shifts from simple unit testing to scenario-based validation, focusing on the complex interactions between the Bayesian
  probability model, the Kalman filter, and the state machine on the Pico 2 (no-FPU) platform.

  ---

  📋 Comprehensive BDD Test Plan: Bus Arrival System v8.0


  1. Test Strategy Overview
   * Engine: cucumber-rust or custom BDD harness in simulator/tests/bdd_localization.rs.
   * Data Source: ground_truth.json for golden-path verification.
   * Focus: Edge cases (GPS jumps, close stops, signal loss, U-turns).
   * Constraint Verification: Ensure all calculations remain within i32/i64 bounds (no overflow).

  ---

  2. Feature: Arrival Detection Logic (Module ⑨–⑪)


  Scenario: Successful Arrival at Standard Stop
  Goal: Verify the 4-feature Bayesian fusion correctly triggers an arrival.
   * Given a route with a stop at progress 10000 cm (100m)
   * And the bus is in the Approaching state
   * When the bus enters the corridor at 9950 cm
   * And the speed drops to 20 cm/s (below 56 cm/s threshold)
   * And the bus remains stationary for 10 seconds
   * Then the probability should exceed 191 (75%)
   * And an ArrivalEvent for that stop must be emitted exactly once.


  Scenario: Skip-Stop Protection (False Positive Prevention)
  Goal: Ensure a bus driving past a stop at high speed does not trigger an arrival.
   * Given a route with a stop at progress 10000 cm
   * When the bus passes the stop at 1111 cm/s (40 km/h)
   * Then the Speed Likelihood (p2) should remain near 0
   * And the dwell_time feature should remain at 0
   * And the state machine must transition from Arriving to Departed without ever entering AtStop.


  Scenario: Ultra-Close Stops Discrimination
  Goal: Verify the system doesn't confuse two stops 80m apart.
   * Given Stop A at 5000 cm and Stop B at 13000 cm (80m gap)
   * When the bus is at 5100 cm
   * Then only Stop A's Corridor Filter should be active
   * And Stop B's state machine must remain in Approaching until the bus passes the delta_sep (20m) safety buffer.

  ---


  3. Feature: GPS Robustness & Recovery (Module ⑥, ⑧, ⑫)


  Scenario: GPS Jump Handling (Recovery)
  Goal: Test the system's ability to resync after a coordinate "teleportation."
   * Given the last known stop was Index 2
   * When a GPS "jump" occurs where progress changes from 20000 cm to 50000 cm (300m jump)
   * Then the Recovery Algorithm must trigger
   * And it should select the closest stop Index >= 1 based on the penalized scoring formula
   * And the system must not emit arrival events for skipped stops.


  Scenario: Dead-Reckoning during Tunnel Outage
  Goal: Verify 10s of signal loss compensation.
   * Given the bus is moving at a stable 1000 cm/s (36 km/h)
   * When GPS status becomes invalid (tunnel entry)
   * Then the system must switch to Dead-Reckoning mode
   * And the KalmanState must increment progress by 1000 cm every second
   * And after 10 seconds, if no signal returns, it must enter GPS_LOST state.

  ---

  4. Feature: Localization Integrity (Module ④–⑦)


  Scenario: Heading-Constrained Map Matching
  Goal: Prevent "snapping" to the wrong side of the road.
   * Given a dual-carriageway with a parallel segment 10m away but heading 180° opposite
   * When the GPS heading is 0° (North)
   * Then the Heading Penalty must be high enough to exclude the opposite segment
   * And even if the GPS coordinate drifts closer to the wrong segment, the score must favor the Northbound segment.

  ---

  5. Non-Functional & Safety Requirements (Senior QA Focus)


  BDD Technical Assertions:
   1. Integer Safety:
       * When calculating dot products for segments at the maximum route extent (e.g., 20km from origin)
       * Then no i64 intermediate values may overflow 9,223,372,036,854,775,807.
   2. Pico 2 Performance:
       * When processing a 1Hz GPS update on a route with 600 nodes and 50 stops
       * Then total CPU execution time must be < 5 ms (budgeting 33% of the 150MHz clock).
   3. Memory Footprint:
       * Given a route of 12km
       * Then the RouteData binary must be < 40 KB and reside in Flash (XIP).

  ---


  6. Execution Plan
   1. Static Analysis: Run cargo clippy with a focus on integer casting (as i32, as i64).
   2. Unit Tests: Run cargo test -p arrival_detector to verify the math logic.
   3. Simulation Replay:
   1     # Replay ground truth data through the arrival detector
   2     ./target/debug/arrival_detector ground_truth.json route_data.bin results.json
   4. Delta Comparison: Compare results.json against ground_truth.json using a Python script to calculate Precision and Recall
      (Target: 97%+).
---

## 7. Active Stops Functionality (2026-03-14)

**Issue Fixed:** Simulator `active_stops` and `stop_states` arrays were always empty for loop routes.

**Root Cause:** GPS was positioned at the loop completion point (s_cm≈1717259) but no stop was placed at that location.

**Solution:**
- Added stop at loop closure point (lat=25.00278, lon=121.28676) to ty225_stops.json
- Modified gen_nmea.js to accept `--stops` parameter for separate stops.json file
- Removed stops array from ty225_route.json to avoid confusion

**Verification:**
- GPS at s_cm=1717259 now shows: `"active_stops":[55],"stop_states":[{"stop_idx":55,"distance_cm":-39}]`
- 10 integration tests added to prevent regression

**Usage:**
```bash
# Generate NMEA with separate stops file
node tools/gen_nmea/gen_nmea.js generate --route route.json --stops stops.json --out_nmea test.nmea --out_gt gt.json

# Run simulator
cargo run -p simulator -- test.nmea route_data.bin output.jsonl

# Verify active_stops are populated
head -1 output.jsonl | grep -o '"active_stops":[^}]*'
```
