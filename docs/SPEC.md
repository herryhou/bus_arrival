# Bus Arrival Detection System - Master Specification

## How to Use This Spec

**For LLMs (Claude, Codex etc.):**
1. Read "Current System Contract" and "Universal Rules" in this file first
2. Read only the module-specific specs listed for your task
3. Treat this file and `docs/specs/*.md` as the source of truth
4. Use `specs/00-constraints.md` as a reference when shared types, budgets,
   constants, or binary layout details are directly relevant

**For Developers:**
- Use these specs as quick reference when implementing
- Update specs when behavior changes

## Current System Contract

This system detects bus stop arrivals from noisy fixed-route GPS data. Runtime
code targets RP2350-class embedded execution: integer-first math, small scalar
state, and 1 Hz GPS updates.

The pipeline has three phases:

1. **Offline preprocessing** converts route and stop GeoJSON into `route_data.bin`
   using route simplification, stop projection, route linearization, and spatial
   grid generation.
2. **Runtime localization** converts NMEA-derived GPS fixes into route progress
   using heading-constrained map matching, speed and monotonicity filters,
   Kalman filtering, dead reckoning, and off-route recovery.
3. **Runtime detection** combines corridor gating, weighted arrival probability,
   a stop state machine, and stop-index recovery to emit `Announce`, `Arrival`,
   and `Departure` domain events plus one-shot stop lifecycle events for
   `Approaching`, `Arriving`, `Arrived`, and `Departed`.

Runtime responsibilities are split by layer:

| Layer | Responsibility |
|---|---|
| Parser | NMEA parsing, timestamp accumulation, GPS fix quality extraction |
| Estimation | GPS-to-route localization, Kalman/DR state, map matching, off-route state |
| Control | Mode transitions and per-tick orchestration |
| Detection | Corridor filtering, probability, FSM, recovery, and event emission |

Route-space position has two distinct signals:

- `z_gps_cm`: raw GPS projection, used for F1 distance evidence.
- `s_cm`: Kalman-filtered route progress, used for F3 progress evidence.

Trace output must expose enough state to debug decisions: GPS input/status,
map-match segment and distance, heading eligibility, `z_gps_cm`, `s_cm`,
`v_cms`, divergence, off-route/recovery state, active stop corridor, probability
features, FSM state, and emitted events.

Android exposes stop lifecycle events through `PipelineResult.Success.stopEvents`
and dispatches them through `StopEventCallback`. The detection pipeline remains
UI-neutral; toast/snackbar/sound behavior belongs in service/viewmodel/UI layers.

## Universal Rules

- Runtime firmware uses integer-first arithmetic. Avoid floating point, `sqrt`,
  `exp`, and trigonometry in hot paths.
- Physical quantities use semantic units: `DistCm`, `SpeedCms`, `HeadCdeg`,
  `GeoCdeg`, `Prob8`, and `Dist2`.
- Use `Dist2`/`i64` for squared distances and dot products.
- Probability values use `Prob8` (`0..255`); weighted sums need wider
  intermediates.
- Runtime state should stay small and scalar: target below 8% CPU at 150 MHz
  and below 1 KB SRAM for core state.
- Route data is generated offline and read from Flash/XIP at runtime.
- Binary layout changes require version bumps and regenerated route binaries.
- Read `specs/00-constraints.md` only when you need detailed shared constants,
  type definitions, resource budgets, or binary layout reference.

## Module Specifications

### Shared Reference
- **`specs/00-constraints.md`** - Cross-cutting constraints reference
  - Semantic type system (DistCm, SpeedCms, HeadCdeg, Prob8)
  - Integer-only arithmetic requirements
  - Memory and CPU budgets
  - XIP (Execute-in-Place) constraints

### Phase 1: Offline Preprocessing
- **`specs/09-preprocessing.md`** - Route preprocessing pipeline
  - Douglas-Peucker simplification
  - Stop-to-segment mapping
  - Binary file generation

- **`specs/10-spatial_index.md`** - Spatial grid index v5.1
  - Sparse bitmask format
  - XIP support for Flash access
  - Binary file structure

### Phase 2: GPS Localization
- **`specs/01-map_matching.md`** - Heading-constrained map matching
  - Spatial grid lookup
  - Heading validation (±90° rule)
  - Candidate selection logic

- **`specs/02-kalman_filter.md`** - 1D Kalman filter
  - State prediction and update
  - Measurement fusion
  - Position uncertainty tracking

- **`specs/03-dead_reckoning.md`** - GPS outage handling
  - Dead-reckoning mode activation
  - Position projection
  - Re-acquisition detection

- **`specs/12-gps_processing.md`** - GPS Processing (Timestamp-Driven)
  - NMEA sentence accumulation
  - Timestamp-driven emission
  - Fix quality classification
  - Split burst handling

### Phase 3: Arrival Detection
- **`specs/04-stop_corridors.md`** - Stop corridor filtering
  - Dynamic corridor sizing
  - Approach corridor vs. at-stop corridor
  - Stop eligibility criteria

- **`specs/05-arrival_probability.md`** - weighted feature arrival model
  - 4-feature probability model
  - Feature distributions (P(d|A), P(v|A), etc.)
  - Threshold tuning

- **`specs/06-state_machine.md`** - Detection state machine
  - State transitions (Approaching → Arriving → AtStop → Departed)
  - Event emission rules
  - One-shot lifecycle events and Android UI-neutral boundary
  - Dwell time tracking

- **`specs/07-stop_recovery.md`** - Stop index recovery
  - Gap detection logic
  - Forward projection algorithm
  - Recovery after GPS anomalies

### Advanced Features
- **`specs/08-off_route_detection.md`** - Off-route detection and recovery
  - Off-route condition detection
  - Position freezing behavior
  - Re-acquisition and recovery

### Human Reference
- **`specs/11-calibration.md`** - Parameter calibration guide
  - Field testing procedures
  - Threshold tuning methodology
  - Performance metrics

## Quick Task → Spec Mapping

| Task | Read These Specs |
|------|------------------|
| Fix map matching bug | 01-map_matching |
| Adjust Kalman parameters | 02-kalman_filter |
| Modify arrival thresholds | 04-stop_corridors, 05-arrival_probability |
| Add new detection feature | 04-06 (detection module) |
| Off-route behavior changes | 08-off_route_detection |
| Binary format changes | 00-constraints, 10-spatial_index |
| GPS outage handling | 03-dead_reckoning |
| GPS processing issues | 12-gps_processing |
| State transition issues | 06-state_machine |
| Stop lifecycle/toast event issues | 06-state_machine, superpowers/specs/2026-05-28-stop-lifecycle-events-design.md |
| Stop index problems | 07-stop_recovery |
| Route preprocessing | 09-preprocessing |
| Performance optimization | 00-constraints (see budgets section) |
| Type system changes | 00-constraints (see semantic types) |

## Specification Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.1 | 2026-05-29 | Documented one-shot stop lifecycle events and Android UI-neutral boundary |
| 1.0 | 2025-04-19 | Initial LLM-spec system created |

## For Spec Authors

When creating or updating module specs:

1. **Follow the template** (if one exists for your module)
2. **Be prescriptive, not descriptive** — specify what the code MUST do
3. **Include concrete examples** with actual values
4. **Document edge cases** explicitly
5. **Reference shared constraints** from 00-constraints.md when relevant
6. **Update this file** when adding new specs

## Related Documentation

- **`CLAUDE.md`** - Project instructions and build commands
- **`spatial_grid_binary_format.md`** - Binary format details
- **`dev_guide.md`** - Embedded Rust development guide
- **`arrival_detector_test.md`** - BDD-style test plan
- **`porting/00-algorithm-spec.md`** - Platform-agnostic algorithm specs, data formats, and testing guides for iOS/ESP32 porting
