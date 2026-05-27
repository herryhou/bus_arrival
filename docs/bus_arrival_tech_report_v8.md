# GPS-Based Bus Arrival Detection System

技術設計報告：Raspberry Pi Pico 2 / RP2350 車載到站判定演算法

| Field | Value |
|---|---|
| Audience | Embedded Rust / Android pipeline developers |
| Current spec version | v9.3, cold-boot acquisition |
| Primary runtime target | Raspberry Pi Pico 2, 1 Hz GPS, no hardware FPU assumed |

## 1. Purpose

This system detects bus arrivals from noisy GPS on a fixed route. It must handle:

- Urban GPS drift of roughly 5-30 m.
- GPS jumps after signal loss, including jumps over stops.
- Stops spaced as closely as 80-120 m.
- GPS outages up to 10 s.
- Pre-arrival voice announcements about 10-15 s before arrival.
- Integer-first embedded execution on RP2350.

The design reduces GPS positions to one-dimensional route progress `s_cm`, then combines corridor gating, probabilistic evidence, and a stop state machine.

## 2. System Overview

The pipeline has three stages.

```text
Offline preprocessing
  route polyline
    -> Douglas-Peucker simplification with stop/curve protection
    -> route linearization: RouteNode[], Stop[]
    -> spatial grid index
    -> route_data.bin in Flash

Runtime localization, 1 Hz
  GPS fix
    -> heading-constrained map matching
    -> projection to route progress z_gps_cm
    -> speed/monotonicity filters
    -> 1D Kalman update or dead reckoning
    -> PositionSignals { z_gps_cm, s_cm }

Runtime detection, 1 Hz
  PositionSignals + stop data
    -> active stop corridor filter
    -> 4-feature arrival probability
    -> stop FSM and stop-index recovery
    -> Announce / Arrival / Departure events
```

Runtime architecture separates responsibilities:

| Layer | Responsibility |
|---|---|
| Parser | NMEA parsing and GPS fix quality extraction. |
| Estimation | GPS -> `PositionSignals`; owns Kalman, DR, map matching, off-route state. |
| Control | Mode transitions and per-tick orchestration. |
| Detection | Corridor filtering, probability, FSM, recovery, events. |

## 3. Units and Data Types

Runtime physical quantities use semantic integer units. Avoid `sqrt`, `exp`, and trigonometry in the hot path; use squared distances, dot products, and LUTs.

| Type | Backing | Unit / range | Use |
|---|---:|---|---|
| `DistCm` | `i32` | centimeters, about +/-214 km | route progress, coordinates |
| `SpeedCms` | `i32` | cm/s | GPS and filtered speed |
| `HeadCdeg` | `i16` | 0.01 degrees, -18000..18000 | headings |
| `GeoCdeg` | `i16` | 0.01 degrees | coarse lat/lon fields |
| `Prob8` | `u8` | 0..255 | likelihoods and probabilities |
| `Dist2` | `i64` | cm squared | dot products, distance squared |

`PositionSignals` intentionally carries two route-space positions:

- `z_gps_cm`: raw GPS projection, used by F1 distance evidence.
- `s_cm`: Kalman-filtered progress, used by F3 progress evidence.

This dual-space contract prevents one noisy signal from silently replacing the other.

## 4. Route Data Format

`route_data.bin` is generated offline and loaded read-only from Flash.

| Field | Current value / layout |
|---|---|
| Magic | `BUSA` |
| Binary `VERSION` | `5` |
| Header | magic, version, node count, stop count, origin, average latitude |
| `RouteNode` | `repr(C)`, 24 bytes |
| `Stop` | `repr(C)`, 12 bytes |
| Grid | sparse bitmask + `u16` offsets + cell index data |
| Integrity | CRC32 over all bytes except trailing checksum |

`RouteNode` layout:

| Field | Type | Unit |
|---|---:|---|
| `x_cm`, `y_cm` | `i32` | absolute projected coordinates |
| `cum_dist_cm` | `i32` | route progress |
| `seg_len_mm` | `i32` | segment length in mm |
| `dx_cm`, `dy_cm` | `i16` | segment vector |
| `heading_cdeg` | `i16` | segment heading |
| `_pad` | `i16` | alignment |

`Stop` layout:

| Field | Type | Meaning |
|---|---:|---|
| `progress_cm` | `DistCm` | stop position along route |
| `corridor_start_cm` | `DistCm` | pre-arrival corridor start |
| `corridor_end_cm` | `DistCm` | post-stop corridor end |

If `RouteNode` or `Stop` layout changes, increment `VERSION` and regenerate every route binary.

## 5. Offline Preprocessing

### 5.1 Polyline Simplification

Use Douglas-Peucker simplification, but preserve:

- Stops and their neighboring route geometry.
- Sharp turns.
- Maximum segment length bounds, so `dx_cm` and `dy_cm` fit `i16`.

The output should reduce Flash use without changing stop order or route geometry enough to affect map matching.

### 5.2 Route Linearization

For each segment, precompute:

- Projected coordinates in centimeters.
- Segment vector and heading.
- Cumulative distance `cum_dist_cm`.
- Segment length in millimeters for more precise projection.

Stops are projected onto the route in sequence. The mapper must preserve monotonic stop order; routes with loops require sequence-aware projection, not a nearest-point greedy match.

### 5.3 Stop Corridors

Default corridor:

- `corridor_start_cm = progress_cm - 8000` (80 m before stop).
- `corridor_end_cm = progress_cm + 4000` (40 m after stop).
- Adjacent corridors keep at least 20 m separation when possible.

For close stops under 120 m apart, split the inter-stop space:

- 55% before the next stop.
- 10% neutral gap.
- 35% after the previous stop.

This keeps the second stop from losing most of its pre-corridor and avoids dwell-time starvation.

### 5.4 Spatial Grid

The grid indexes route segments by 100 m cells. Runtime queries nearby cells and then ranks candidate segments. Grid format v5 stores only non-empty cells through a bitmask and `u16` offsets.

## 6. Map Matching

### 6.1 Candidate Search

Runtime converts GPS lat/lon to projected centimeters using the route average latitude, queries the grid near the GPS point, and scores candidate segments by squared perpendicular distance.

Projection uses integer dot products:

```text
t = clamp(dot(P - A, B - A) / |B - A|^2, 0, 1)
z_gps_cm = A.cum_dist_cm + t * segment_length
```

All distance ranking uses squared distance; no `sqrt` is required.

### 6.2 Heading Filter

Map matching uses filter-then-rank:

1. Filter candidates with `heading_eligible`.
2. Rank eligible candidates by distance only.
3. Fall back to the best distance candidate only when no candidate passes the heading filter.

Heading rules:

| Condition | Rule |
|---|---|
| Missing heading sentinel `i16::MIN` | Always eligible. |
| Stopped | Heading gate disabled. |
| Moving at >= 83 cm/s (3 km/h) | Max heading difference 90 degrees. |
| Between 0 and 83 cm/s | Threshold tightens linearly. |
| First fix / recovery / cold boot | Relaxed 180 degree threshold. |

Do not mix heading degrees into the distance score. Heading is a boolean physical-plausibility gate.

## 7. Localization Filters

### 7.1 Speed Constraint

Reject a projected GPS update when:

```text
abs(z_new - s_prev) > V_MAX_CMS * max(dt, 1) + SIGMA_GPS_CM
```

Current constants:

- `V_MAX_CMS = 1667` (60 km/h).
- `SIGMA_GPS_CM = 2000` (20 m GPS margin).

Rejected updates run a predict-only step, equivalent to short dead reckoning.

### 7.2 Monotonicity

Reject route progress that jumps backward more than 50 m:

```text
z_new >= s_prev - 5000
```

Exactly -50 m is allowed.

## 8. Kalman and Dead Reckoning

The runtime state estimate is one-dimensional:

```text
s_pred = s_cm + v_cms
v_pred = v_cms
s_cm   = s_pred + ks * (z_gps_cm - s_pred) / 256
v_cms  = max(0, v_pred + 77 * (v_gps_cms - v_pred) / 256)
```

Position gain `ks` depends on accuracy when available, otherwise HDOP, otherwise poor quality:

| Quality source | Range | `ks` |
|---|---:|---:|
| accuracy | `< 800 cm` | 128 |
| accuracy | `800..=2000 cm` | 51 |
| accuracy | `2001..=5000 cm` | 26 |
| accuracy | `> 5000 cm` | 13 |
| HDOP x10 | `0..=20` | 77 |
| HDOP x10 | `21..=30` | 51 |
| HDOP x10 | `31..=50` | 26 |
| HDOP x10 | `> 50` | 13 |

Dead reckoning is used when GPS has no fix or a fix is rejected:

- `s_cm = last_valid_s + filtered_v * dt`.
- `filtered_v` is an EMA: `v += 3 * (v_gps - v) / 10`.
- During outage, velocity decays by `(0.9)^dt` through an integer LUT.
- Outages over 10 s return `Outage` and set recovery mode for the next valid fix.
- First post-outage fix uses relaxed heading and soft resync: 20% position/velocity correction.

## 9. Cold-Boot Acquisition

Cold boot is explicit state: `KalmanState.is_cold_boot = true`.

While cold boot is active:

1. Match GPS with first-fix/recovery heading mode (`180` degree threshold).
2. Require two consecutive good matches:
   - `match_d2 <= 25_000_000` (within 50 m squared).
   - heading constraint passes.
3. Return `ProcessResult::Acquiring` until the counter reaches two.
4. Snap `s_cm` to the current GPS projection and clear `is_cold_boot`.

Cold boot does not use the off-route re-entry `min_s` constraint because there is no trusted prior route progress.

## 10. Off-Route Detection and Recovery

Off-route detection uses map-match distance squared:

| Constant | Value |
|---|---:|
| `OFF_ROUTE_D2_THRESHOLD` | `25_000_000 cm^2` (50 m) |
| `OFF_ROUTE_CONFIRM_TICKS` | `5` |
| `OFF_ROUTE_CLEAR_TICKS` | `2` |

Behavior:

- On the first suspect tick, freeze `s_cm` and capture freeze context.
- During suspect/off-route, skip projection and Kalman updates so route progress does not advance through a detour.
- After two good matches, re-enter normal mode and snap to a forward position.
- Re-entry grid search is constrained to `frozen_s_cm..frozen_s_cm + 500_000` (up to 5 km forward).
- Backward re-entry snaps are rejected.

Stop recovery after jumps searches stops near the current `s_cm`:

- Candidate must be within 200 m.
- Candidate index must be at least `last_index - 1`.
- Backward recovery over 100 m is rejected.
- Forward candidates must be reachable from filtered speed and elapsed time.
- Score combines distance, backward-index penalty, and optional freeze-anchor penalty.

## 11. Stop Detection

### 11.1 Active Stop Filter

A stop is active when the estimated route progress is inside its corridor:

```text
corridor_start_cm <= s_cm <= corridor_end_cm
```

Corridor entry triggers an `Announce` event once per stop when the stop FSM is active.

### 11.2 Probability Model

Arrival probability is a weighted fusion of four `Prob8` features:

| Feature | Signal | Model |
|---|---|---|
| F1 | `abs(z_gps_cm - stop.progress_cm)` | Gaussian LUT, `sigma = 2750 cm` |
| F2 | `v_cms` | Logistic LUT, stop speed center `200 cm/s` |
| F3 | `abs(s_cm - stop.progress_cm)` | Gaussian LUT, `sigma = 2000 cm` |
| F4 | `dwell_time_s` | linear ramp to 255 at 10 s |

Standard fusion:

```text
P = (13*F1 + 6*F2 + 10*F3 + 3*F4) / 32
THETA_ARRIVAL = 191
```

Close-stop fusion, when the next sequential stop is under 120 m away:

```text
P = (14*F1 + 7*F2 + 11*F3 + 0*F4) / 32
```

Protection rules:

- If `GpsStatus::OffRoute`, probability is `0`.
- During `DrOutage` or `OffRoute` with `divergence > PHANTOM_DIVERGENCE_CM`, neutralize F1 and F3 to `128`.
- When GPS is valid but `abs(z_gps_cm - s_cm) > 2000`, F1 may use `s_cm` defensively to avoid a bad projection dominating the decision.

### 11.3 Stop State Machine

States:

```text
Idle -> Approaching -> Arriving -> AtStop -> Departed -> TripComplete
```

Transition rules:

| From | To | Condition |
|---|---|---|
| `Idle` | `Approaching` | `s_cm >= corridor_start_cm` |
| `Approaching` | `Arriving` | `abs(s_cm - stop.progress_cm) < 5000` |
| `Arriving` | `AtStop` | distance `< 5000` and `P > 191` |
| `Arriving`/`AtStop` | `Departed` | distance `> 4000` and `s_cm > stop.progress_cm` |
| `Approaching`/`Arriving` | `Idle` | `s_cm < corridor_start_cm` |

Rules:

- No speed threshold is required for arrival confirmation.
- Probability must be strictly greater than `191`; equality does not trigger.
- Dwell time starts when entering `Approaching` and resets on corridor exit before arrival.
- Each stop can announce/arrive only once per trip; `Departed` does not reactivate.
- Final stop departure moves to terminal trip-complete handling.

## 12. Trace and Outputs

The trace should expose enough state to debug decisions:

- GPS input and status.
- Map-match segment, distance squared, heading eligibility.
- `z_gps_cm`, `s_cm`, `v_cms`, and divergence.
- Off-route status, counters, freeze position, snap events.
- Active stop corridor, probability, feature scores, FSM state.
- Emitted `Announce`, `Arrival`, and `Departure` events.

## 13. Performance Targets

Expected RP2350 runtime budget at 1 Hz:

| Component | Target cost |
|---|---:|
| Map matching with grid | O(k), usually 5-10 candidate segments |
| Kalman / filters / DR | constant time |
| Stop corridor and probability | O(active stops), usually 0-2 |
| CPU | below 8% at 150 MHz |
| Runtime SRAM | below 1 KB for core state |
| Route Flash | about 20-35 KB, route dependent |

Route binaries dominate Flash use. Runtime state is intentionally small and mostly scalar.

## 14. Embedded Rust Requirements

- Keep hot-path math integer-first.
- Use `u16`/`u32` intermediates for weighted probability; `32 * 255 = 8160` does not fit `u8`.
- Use `i64` for squared distances and dot products.
- Clamp velocity to non-negative after Kalman updates.
- Use unaligned reads for Flash/XIP binary access.
- Treat binary layout assertions as load-bearing.
- Keep mode transitions single-step per tick; avoid competing recovery paths.

## 15. HMM Map Matching

HMM/Viterbi map matching is an optional future path for dense urban or loop-heavy routes. It should remain offline or bounded-window unless profiling proves runtime budget is available.

If implemented:

- Emission: distance Gaussian LUT.
- Transition: difference between route distance and GPS movement.
- Candidate set: fixed grid window.
- State count: small bounded `K`; no unbounded dynamic allocation on firmware.

The current production design uses deterministic filter-then-rank map matching.

## 16. Verification

Required scenario coverage:

| Scenario | Expected result |
|---|---|
| Normal route | Stops announced and arrived in order. |
| Close stops | No missed arrivals due to short pre-corridor. |
| GPS drift | No duplicate arrivals; no false stop reactivation. |
| GPS outage <= 10 s | DR continues, recovery resyncs without jumping backward. |
| Long outage | `Outage` then relaxed-heading recovery on next valid fix. |
| Off-route detour | Progress freezes, skipped stops are not falsely arrived. |
| Cold boot | Returns `Acquiring` until two good matches, then snaps to route. |
| DR phantom arrival | F1/F3 protection prevents false arrival when raw GPS is far away. |

Relevant tests include Rust pipeline/gps/detection unit tests and scenario golden tests under `crates/pipeline/tests/` plus Android parity tests under `android/app/src/test/`.

## Appendix A. Parameter Reference

| Parameter | Value |
|---|---:|
| GPS loop period | 1 Hz |
| Default pre-corridor | 8000 cm |
| Default post-corridor | 4000 cm |
| Corridor separation target | 2000 cm |
| Close-stop threshold | 12000 cm |
| Arrival distance | `< 5000 cm` |
| Departure distance | `> 4000 cm` after stop |
| Arrival probability threshold | `P > 191` |
| Gaussian F1 sigma | 2750 cm |
| Gaussian F3 sigma | 2000 cm |
| Dwell full scale | 10 s |
| Speed logistic center | 200 cm/s |
| Max city bus speed | 1667 cm/s |
| GPS speed-filter margin | 2000 cm |
| Backward monotonic tolerance | 5000 cm |
| Off-route distance squared | 25,000,000 cm^2 |
| Off-route confirm / clear | 5 / 2 ticks |
| Cold-boot good matches | 2 consecutive ticks |
| Recovery search forward cap | 500,000 cm |
| DR outage supported duration | 10 s |

## Appendix B. Changelog Summary

Only current behavior belongs in the main spec. Historical details are summarized here for context:

- v9.3: Added cold-boot acquisition and `ProcessResult::Acquiring`.
- v9.2: Added DR/off-route phantom-arrival protection for F1/F3.
- v9.1: Raised excellent GPS Kalman position gain to `128/256`.
- v9.0: Split parser, estimation, control, recovery, and detection responsibilities.
- v8.9: Added off-route hysteresis, position freezing, and re-entry snap.
- v8.8: Changed map matching to heading filter-then-distance-rank.
- v8.6: Added close-stop probability/corridor handling and one-time stop arrival rule.
- v8.5: Fixed binary layout, probability overflow, FSM edges, monotonicity, and Kalman velocity clamp.
- v8.4: Added corridor-entry voice announcements.
- v8.3-v8.1: Improved stop projection, route binary layout, and arrival FSM thresholds.
