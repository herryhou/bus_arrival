# Map Matching Test Coverage & Refactoring Design

**Date:** 2026-05-02
**Status:** Approved
**File:** `crates/pipeline/gps_processor/src/map_match.rs`

## Overview

**Goal:** Improve reliability and maintainability of `map_match.rs` through comprehensive test coverage and targeted refactoring.

**Scope:** Two-phase approach
- **Phase 1:** Add comprehensive test coverage (property-based + edge cases)
- **Phase 2:** Refactor duplicated code + improve documentation

**Success Criteria:**
- All 7 untested functions have test coverage
- Property-based tests for math functions catch edge cases
- Global search fallback extracted into helper (eliminates duplication)
- Undocumented constants have rationale documented
- All existing tests continue to pass

## Problem Statement

The `map_match.rs` file has two reliability concerns:

1. **Code Duplication:**
   - Global search fallback appears twice verbatim (lines 188-201, 207-220)
   - Grid search pattern repeated 3× with semantic differences

2. **Test Coverage Gap:**
   - Only 4 tests covering 2 functions
   - 7 functions lack any tests
   - No edge case coverage
   - No property-based tests for math functions

## Design Decisions

### Refactoring Scope

**Decision:** Minimal extraction (Option B from brainstorming)

Extract only the global search fallback into a helper function. Leave the 3× grid search code as-is.

**Rationale:**
- The global search fallback is pure duplication (identical code, identical behavior)
- The grid searches are semantically different (seed vs no-seed, filter vs no-filter)
- Merging grid searches would hide intent behind abstraction
- YAGNI principle - no current bug reports affecting all three grid searches

### Testing Strategy

**Decision:** Tests-first approach with comprehensive coverage

**Rationale:**
- Tests become a refactoring safety net
- Catches regressions immediately
- Documents expected behavior
- Enables confident refactoring

### Testing Framework

**Decision:** Use `proptest` for property-based tests

**Rationale:**
- Standard Rust property testing framework
- Sophisticated shrinking for minimal counterexamples
- Excellent for catching edge cases in math functions

### Test Data Approach

**Decision:** Minimal mock builder (`RouteDataBuilder`)

**Rationale:**
- Fast, deterministic tests
- Full control over test scenarios
- No dependency on external files
- Easy to create specific edge cases

## Implementation Plan

### Phase 1: Test Infrastructure

**Dependencies:** Add `proptest` to dev-dependencies in `crates/pipeline/gps_processor/Cargo.toml`

**Test Utilities Module:** Create test utilities inline in `map_match.rs` test module:
- `RouteDataBuilder` - Fluent builder for creating test `RouteData`
  - Configurable grid size, cell size
  - Add segments with position, heading, length
  - Auto-populate grid cells
  - Build edge cases (empty route, single segment, etc.)

### Phase 2: Test Implementation

**2.1 Math Helper Tests (with proptest)**
- `heading_threshold_cdeg` - Test interpolation curve, boundary at w=0 and w=256
- `heading_diff_cdeg` - Test 360° wraparound, reflex angles (>180°)
- `distance_to_segment_squared` - Test clamped projection, zero-length segments, point before/after/on segment
- `project_to_route` - Test projection returns valid `cum_dist_cm`

**2.2 Public API Tests (using RouteDataBuilder)**
- `find_best_segment_restricted` - Test window search early exit, grid fallback, global fallback
- `find_best_segment_grid_only` - Test off-route recovery, grid boundary cases
- `find_best_segment_grid_only_with_min_s` - Test backward-snap prevention

**2.3 Edge Case Tests**
- Empty routes (node_count = 0)
- Single-segment routes
- GPS outside grid bounds (all 4 directions)
- Sentinel heading (`i16::MIN`) propagation
- First-fix mode (`is_first_fix = true`)
- Zero speed (w=0, heading gate disabled)

**2.4 Property-Based Test Strategy**
- `heading_diff_cdeg`: `diff(a,b) == diff(b,a)`, `diff(a,a) == 0`, `diff(a,b) <= 18000`
- `distance_to_segment_squared`: Always non-negative, zero when point is on segment

### Phase 3: Refactoring

**3.1 Extract Global Search Fallback**

Create a new helper function:
```rust
/// Global search fallback when GPS is outside grid bounds.
/// Searches all segments and returns the best eligible (or best any if none eligible).
fn global_search_fallback(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
) -> (usize, Dist2) {
    let start = 0;
    let end = route_data.node_count.saturating_sub(1);
    let (best_eligible, eligible_dist2, eligible_found,
         best_any, any_dist2) = best_eligible(
        gps_x, gps_y, gps_heading, gps_speed, route_data, start..=end, is_first_fix
    );
    if eligible_found {
        (best_eligible, eligible_dist2)
    } else {
        (best_any, any_dist2)
    }
}
```

Replace the two duplicated blocks (lines 188-201 and 207-220) with calls to `global_search_fallback(...)`.

**3.2 Document Constants**

Add documentation for `WINDOW_BACK` and `WINDOW_FWD`:
```rust
/// Window search looks back 2 segments and forward 10 segments from last_idx.
/// These values are derived from:
/// - GPS update rate: 1 Hz
/// - Typical bus speed: 30-50 km/h (~8-14 m/s)
/// - Segment length: ~20 m on average
/// - In one second, a bus travels ~8-14 m, or ~0.4-0.7 segments
/// - Window of ±10 segments provides ~20 second buffer for GPS outliers
const WINDOW_BACK: usize = 2;
const WINDOW_FWD: usize = 10;
```

**Note:** The actual rationale will be verified during implementation by checking git history or docs. If not found, will document the current best understanding.

**3.3 Cleanup Opportunities**

During implementation, check for:
- Unused imports
- Dead code (no callers)
- Opportunities to consolidate helper functions

## Implementation Order

1. **Phase 1:** Test Infrastructure
   - Add `proptest` dependency
   - Create `RouteDataBuilder` in test module
   - Set up test utilities

2. **Phase 2:** Test Implementation
   - Math helper tests with proptest
   - Public API tests using RouteDataBuilder
   - Edge case tests
   - Verify all tests pass

3. **Phase 3:** Refactoring
   - Extract `global_search_fallback` helper
   - Replace duplicated blocks
   - Add constant documentation
   - Cleanup opportunities
   - Verify all tests still pass

4. **Phase 4:** Validation
   - Run full test suite
   - Run existing integration tests
   - Verify no behavior change

## Success Criteria

- [ ] All 7 untested functions have test coverage
- [ ] Property-based tests cover edge cases for math functions
- [ ] Global search duplication eliminated (reduced by ~28 lines)
- [ ] `WINDOW_BACK` and `WINDOW_FWD` documented with rationale
- [ ] All existing tests pass
- [ ] No behavior change (integration tests pass)
- [ ] Code review completed

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking existing behavior | Tests first - any change is caught immediately |
| RouteDataBuilder doesn't cover real cases | Start simple, extend as needed during testing |
| proptest flakiness | Use deterministic seeds for reproducibility |
| Constant documentation wrong | Mark as "best current understanding", verify via git history |

## Related Files

- `crates/pipeline/gps_processor/src/map_match.rs` — Implementation
- `docs/specs/01-map_matching.md` — Map matching specification
- `crates/pipeline/gps_processor/Cargo.toml` — Add proptest dependency

## Version History

- 2026-05-02: Initial design
