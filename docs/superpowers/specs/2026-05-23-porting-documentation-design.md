# Porting Documentation Design

**Date:** 2026-05-23
**Status:** Approved
**Target:** Senior devs porting bus arrival detection to iOS/ESP32

## Goals

Create platform-agnostic documentation for porting the bus arrival detection system to iOS and ESP32 platforms.

1. **Algorithm understanding** - Clear explanation of 3-phase pipeline without platform-specific details
2. **Data format integration** - Complete specs for trace_v2.jsonl, binary route format, NMEA input
3. **Testing/validation** - Multi-layer testing strategy to ensure ≥97% accuracy

## Scope

**In scope:**
- 5 modular documentation files in `docs/porting/`
- Algorithm specification (math-agnostic)
- Data format documentation
- Testing strategy (component + golden + BDD)
- Platform-specific checklists (iOS/ESP32)

**Out of scope:**
- Platform-specific API documentation (CoreLocation, FreeRTOS)
- Code translation (Rust → Swift/C++)
- Production deployment guides

## Architecture Overview

### 3-Phase Pipeline

```
Input: NMEA + Route
    │
    ▼
Phase 1: Map Matching
  • Spatial grid search (heading-constrained)
  • Find best route segment for GPS position
  Output: segment_idx, heading_constraint_met
    │
    ▼
Phase 2: State Estimation
  • 1D Kalman Filter (s_cm: position along route)
  • Dead-Reckoning (GPS outage handling)
  • Divergence detection (off-route trigger)
  Output: s_cm, v_cms, variance_cm2, divergence_cm
    │
    ▼
Phase 3: Arrival Detection
  • Stop Corridor (spatial filter)
  • Bayesian Probability Model (4 features)
  • State Machine (Approaching → Arriving → AtStop)
  Output: ArrivalEvent, FSM state per stop
    │
    ▼
Output: trace_v2.jsonl (per-tick complete state)
```

**Key insight for porting:** Each phase has well-defined inputs/outputs. Implement in any language/platform.

### Core Data Types

| Type | Semantic | Range | Purpose |
|------|----------|-------|---------|
| `s_cm` | Position along route | ±214 km | 1D coordinate |
| `v_cms` | Velocity along route | 0..214 km/h | Speed |
| `DistCm` | Distance in cm | ±214 km | GPS distances |
| `HeadCdeg` | Heading | -180°..+180° | Direction |
| `Prob8` | Probability | 0..255 | Arrival confidence |

## Documentation Structure

### `docs/porting/00-algorithm-spec.md`

**Purpose:** Platform-agnostic algorithm description

**Contents:**
- Phase 1: Map matching
  - Spatial grid search algorithm
  - Heading-constrained filtering
  - Projection math (z_cm calculation)
- Phase 2: State Estimation
  - 1D Kalman filter equations
  - Dead-reckoning logic
  - Divergence detection (off-route)
- Phase 3: Arrival Detection
  - Stop corridor definition
  - 4-feature probability model
  - FSM states and transitions
- Platform notes
  - Where integer math matters (no-FPU platforms)
  - Where floating-point is acceptable

### `docs/porting/01-data-formats.md`

**Purpose:** Complete data format specifications

**Contents:**
- NMEA input format
  - RMC sentence (position, heading, speed)
  - GGA sentence (HDOP, time)
- Binary route format
  - RouteNode structure (precomputed coefficients)
  - Stop structure (corridor definition)
  - Spatial grid index
  - XIP alignment requirements
- trace_v2.jsonl output format
  - Per-tick JSON line structure
  - GPS state, Kalman state, detection state
  - Stop states with probability features
- Ground truth JSON format
  - Arrival validation format
  - stop_idx, timestamp, dwell_s

### `docs/porting/02-component-testing.md`

**Purpose:** Per-component test criteria

**Contents:**
- Map matching tests
  - Segment selection accuracy
  - Heading constraint filtering
  - Projection correctness
  - Test vectors
- Kalman filter tests
  - Convergence criteria
  - Divergence threshold validation
  - Dead-reckoning accuracy
  - Test vectors
- Detection tests
  - Probability calibration
  - FSM transition correctness
  - Corridor filtering
  - Test vectors

### `docs/porting/03-golden-tests.md`

**Purpose:** End-to-end validation procedure

**Contents:**
- Test data catalog
  - ty225_normal (baseline)
  - ty225_jump (GPS jump recovery)
  - ty225_drift (GPS drift handling)
- Validation procedure
  - Feed same NMEA to ported implementation
  - Compare arrivals vs ground truth
  - Trace diff for debugging
- Accuracy metrics
  - ≥97% arrival detection target
  - False positive/negative tolerance

### `docs/porting/04-porting-checklist.md`

**Purpose:** Platform-specific considerations

**Contents:**
- iOS porting
  - CoreLocation integration
  - Floating-point vs integer math tradeoffs
  - Threading model (OperationQueue, async/await)
  - Memory management
- ESP32 porting
  - GPS driver integration (UART NMEA)
  - Memory constraints (SRAM budget)
  - FreeRTOS task structure
  - Flash storage for route data
- Common tasks
  - NMEA parsing
  - Route data loading
  - Trace output generation

## Testing Strategy

**Three-layer approach:**

1. **Component specs** - Each phase independently testable with defined pass criteria
2. **Golden tests** - End-to-end validation against Rust reference implementation
3. **BDD scenarios** - Edge case coverage (GPS jumps, close stops, detours)

**For iOS/ESP32 porting:**
- iOS: Use floating-point math, verify accuracy vs integer reference
- ESP32: Verify memory constraints, validate integer math
- Both: Golden tests ensure algorithm porting accuracy

## Implementation Notes

- Docs will reference existing v9 tech report for mathematical details
- Trace examples from test_data/ will be included
- Component test vectors extracted from existing tests
- Platform checklists based on common porting patterns

## Success Criteria

- Senior dev can understand algorithm without reading Rust code
- Data formats are fully specified for independent implementation
- Testing procedure ensures ≥97% accuracy on ported platforms
- Platform-specific considerations are clearly documented
