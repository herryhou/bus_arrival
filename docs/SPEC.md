# Bus Arrival Detection System - Master Specification

## How to Use This Spec

**For LLMs (Claude, etc.):**
1. ALWAYS read `specs/00-constraints.md` first — it applies to ALL modules
2. Read module-specific specs before working on that module
3. Do NOT rely on `bus_arrival_tech_report_v8.md` for implementation rules

**For Developers:**
- Use these specs as quick reference when implementing
- Read `bus_arrival_tech_report_v8.md` for algorithm background
- Update specs when behavior changes

## Module Specifications

### Mandatory Pre-Reading
- **`specs/00-constraints.md`** - Cross-cutting constraints (MUST read first)
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

### Phase 3: Arrival Detection
- **`specs/04-stop_corridors.md`** - Stop corridor filtering
  - Dynamic corridor sizing
  - Approach corridor vs. at-stop corridor
  - Stop eligibility criteria

- **`specs/05-arrival_probability.md`** - Bayesian arrival model
  - 4-feature probability model
  - Feature distributions (P(d|A), P(v|A), etc.)
  - Threshold tuning

- **`specs/06-state_machine.md`** - Detection state machine
  - State transitions (Approaching → Arriving → AtStop → Departed)
  - Event emission rules
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
| Fix map matching bug | 00-constraints, 01-map_matching |
| Adjust Kalman parameters | 00-constraints, 02-kalman_filter |
| Modify arrival thresholds | 00-constraints, 04-stop_corridors, 05-arrival_probability |
| Add new detection feature | 00-constraints, 04-06 (detection module) |
| Off-route behavior changes | 00-constraints, 08-off_route_detection |
| Binary format changes | 00-constraints, 10-spatial_index |
| GPS outage handling | 00-constraints, 03-dead_reckoning |
| State transition issues | 00-constraints, 06-state_machine |
| Stop index problems | 00-constraints, 07-stop_recovery |
| Route preprocessing | 00-constraints, 09-preprocessing |
| Performance optimization | 00-constraints (see budgets section) |
| Type system changes | 00-constraints (see semantic types) |

## Specification Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-04-19 | Initial LLM-spec system created |

## For Spec Authors

When creating or updating module specs:

1. **Follow the template** (if one exists for your module)
2. **Be prescriptive, not descriptive** — specify what the code MUST do
3. **Include concrete examples** with actual values
4. **Document edge cases** explicitly
5. **Reference constraints** from 00-constraints.md
6. **Update this file** when adding new specs

## Related Documentation

- **`CLAUDE.md`** - Project instructions and build commands
- **`bus_arrival_tech_report_v8.md`** - Complete system design and algorithms
- **`spatial_grid_binary_format.md`** - Binary format details
- **`dev_guide.md`** - Embedded Rust development guide
- **`arrival_detector_test.md`** - BDD-style test plan
