# Pipeline Module Refactoring Design

**Date:** 2026-05-09
**Status:** Draft
**Goals:**
1. Prepare for firmware extraction (embedded constraints)
2. Improve testability (unit tests for components)
3. Fix code hygiene issues

---

## Problem Statement

The `pipeline` crate has 5 technical debt issues identified via code review:

1. **PhantomData anti-pattern** - `LocalizationState<'a>` uses `PhantomData<&'a ()>` but doesn't hold reference
2. **Duplicate computation** - `get_trace_info()` recomputes probability/features already computed in `process_gps_record()`
3. **Corridor filtering** - Should be separate component, currently inline in `DetectionState`
4. **7-parameter diagnostics** - `.with_diagnostics(a,b,c,d,e,f,g)` violates clean interface
5. **Magic numbers** - `10000` should be named constant

---

## Architecture

### Current State
```
pipeline/src/
├── lib.rs              (570 lines) - Pipeline, orchestration
├── localization.rs     (108 lines) - LocalizationState
├── detection_state.rs  (211 lines) - DetectionState (mixed concerns)
└── trace.rs            (70 lines)  - Trace types
```

### Target State
```
pipeline/src/
├── lib.rs              - Public API, orchestration
├── localization.rs     - LocalizationState (lifetime-free)
├── detection.rs        - DetectionState (delegated to filter/probability)
├── filter.rs           - CorridorFilter (standalone)
└── trace.rs            - Trace types

pipeline/filter/        (new crate)
└── lib.rs              - Corridor filtering, active stop selection

pipeline/probability/   (new crate)
└── lib.rs              - Probability computation, feature scoring
```

---

## Component Interfaces

### CorridorFilter (pipeline/filter crate)

Pure function for finding active stops:

```rust
use shared::{DistCm, binfile::RouteData, Stop};

/// Find stops within corridor of current position
pub fn active_stops(
    s_cm: DistCm,
    stops: &[Stop],
    skip_flags: &[bool]
) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(idx, stop)| {
            !skip_flags[*idx]
                && s_cm >= stop.corridor_start_cm
                && s_cm <= stop.corridor_end_cm
        })
        .map(|(idx, _)| idx)
        .collect()
}
```

### ProbabilityEngine (pipeline/probability crate)

Cached probability computation:

```rust
use shared::{DistCm, SpeedCms, PositionSignals, binfile::Stop};
use detection::probability::GpsStatus;

/// Cached probability computation result
#[derive(Debug, Clone)]
pub struct ProbabilityResult {
    pub probability: u8,
    pub features: detection::trace::FeatureScores,
}

/// Compute arrival probability (cached)
pub struct ProbabilityEngine {
    last_result: Option<(u64, ProbabilityResult)>,
}

impl ProbabilityEngine {
    pub fn new() -> Self {
        Self { last_result: None }
    }

    pub fn compute(
        &mut self,
        timestamp: u64,
        signals: PositionSignals,
        v_cms: SpeedCms,
        stop: &Stop,
        dwell_time_s: u16,
        gps_status: GpsStatus,
    ) -> &ProbabilityResult;
}
```

### GpsDiagnostics (localization.rs)

Replaces 7-parameter tuple:

```rust
pub struct GpsDiagnostics {
    pub segment_idx: Option<u16>,
    pub heading_met: bool,
    pub divergence_cm: i32,
    pub hdop: Option<f32>,
    pub num_sats: Option<u8>,
    pub fix_type: Option<String>,
    pub variance_cm2: i32,
}

impl GpsRecord {
    pub fn with_diagnostics(mut self, diag: GpsDiagnostics) -> Self;
}
```

### Constants (detection.rs)

```rust
pub const DETOUR_JUMP_THRESHOLD_CM: i32 = 10000;
```

---

## Data Flow

### Current Flow (with issues)
```
process_gps_record()
├── Update off-route state
├── Find active stops (inline corridor logic)  ← Issue: mixed concern
├── For each active stop:
│   ├── Compute probability (inline)           ← Issue: duplicated later
│   ├── Update state machine
│   └── Handle events
└── get_trace_info()
    └── Recompute probability + features       ← Issue: duplicate computation
```

### New Flow (clean)
```
process_gps_record()
├── Update off-route state
├── active = filter::active_stops()            ← Pure function
├── For each active stop:
│   ├── result = prob_engine.compute()         ← Cached
│   ├── Update state machine
│   └── Handle events
└── get_trace_info()
    └── Use cached probability results         ← No recomputation
```

### DetectionState Changes

```rust
pub struct DetectionState {
    stop_states: Vec<StopState>,
    prob_engine: probability::ProbabilityEngine,  // ← New
    arrived_this_frame: Vec<u8>,
    active_indices: Vec<usize>,
    off_route: bool,
    off_route_last_s_cm: Option<DistCm>,
}
```

---

## Error Handling

All components are fallible-free:
- `filter::active_stops()` - returns `Vec<usize>` (empty if none)
- `ProbabilityEngine::compute()` - returns `&ProbabilityResult` (always valid)
- `LocalizationState::process_gps()` - returns `Option<GpsRecord>` (existing)

No new error types needed.

---

## Testing Strategy

### Phase 1: Characterization Tests (before any changes)

Capture current behavior to prevent regressions:

```rust
// pipeline/tests/characterization.rs
#[test]
fn test_pipeline_full_run() {
    let result = Pipeline::process_nmea_file(
        "tests/data/ty225_normal.nmea",
        "tests/data/ty225.bin",
    ).unwrap();

    // Characterize: arrival count
    assert_eq!(result.arrivals.len(), 11); // ty225 normal scenario

    // Characterize: first arrival details
    let first = &result.arrivals[0];
    assert_eq!(first.stop_idx, 0);
    assert!(first.s_cm > 0);

    // Characterize: departure count
    assert_eq!(result.departures.len(), 11);
}
```

### Phase 2: Unit Tests (as we extract)

```rust
// pipeline/filter/tests/filter_test.rs
#[test]
fn test_active_stops_single() {
    let stops = mock_stops();
    let skip = vec![false; stops.len()];
    let active = filter::active_stops(5000, &stops, &skip);
    assert_eq!(active, vec![0]);
}

#[test]
fn test_active_stops_skip_flag() {
    let stops = mock_stops();
    let skip = vec![true, false];
    let active = filter::active_stops(5000, &stops, &skip);
    assert_eq!(active, vec![1]);
}
```

```rust
// pipeline/probability/tests/cache_test.rs
#[test]
fn test_probability_caching() {
    let mut engine = ProbabilityEngine::new();
    let signals = PositionSignals::new(100, 100);

    let r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
    let r2 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
    assert_eq!(r1 as *const _, r2 as *const _); // Same ptr
}
```

### Phase 3: Integration

Run existing test suite (42 tests) after each phase to verify no regressions.

---

## Implementation Phases

### Phase 1: Quick Wins
- Add `DETOUR_JUMP_THRESHOLD_CM` constant
- Remove PhantomData lifetime from LocalizationState
- Write characterization test
- Run tests

### Phase 2: Extract CorridorFilter
- Create `pipeline/src/filter.rs`
- Write unit tests for `active_stops()`
- Update DetectionState to use filter
- Run tests

### Phase 3: Extract ProbabilityEngine
- Create `pipeline/src/probability.rs`
- Implement caching
- Write unit tests
- Update DetectionState
- Fix duplicate computation in get_trace_info()
- Run tests

### Phase 4: Fix Diagnostics Interface
- Create `GpsDiagnostics` struct
- Update GpsRecord::with_diagnostics()
- Update localization.rs
- Run tests

### Phase 5: Move to Separate Crates
- Create `pipeline/filter/` crate
- Create `pipeline/probability/` crate
- Move code
- Update imports
- Final test run

---

## Success Criteria

1. All 42 existing tests pass
2. New unit tests for filter and probability modules
3. Characterization test guards against regressions
4. No duplicate computation (cached probability results)
5. Clean interfaces (structs, not tuples)
6. Code ready for firmware extraction
