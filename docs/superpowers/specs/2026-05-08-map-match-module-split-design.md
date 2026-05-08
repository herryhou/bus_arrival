# Map Match Module Split Design

**Date:** 2026-05-08
**Status:** Draft
**File:** `crates/pipeline/gps_processor/src/map_match/`

## Overview

**Goal:** Improve testability and reduce cognitive load of `map_match.rs` (1093 lines) by splitting into focused, independently testable modules.

**Scope:** Refactor into 4 modules by concern
- `heading.rs` - Heading filter logic
- `projection.rs` - Distance calculation and route projection
- `search.rs` - Search strategies and orchestration
- `mod.rs` - Public API and re-exports

**Success Criteria:**
- Each module < 300 lines and single-purpose
- Heading and projection modules testable without RouteData fixtures
- All existing tests pass without modification
- Public API unchanged (backward compatible)
- Integration tests produce identical output

## Problem Statement

The `map_match.rs` file has 1093 lines mixing multiple concerns:

1. **Heading filtering** (~100 lines): `heading_eligible`, `heading_threshold_cdeg`, `heading_weight`, `heading_diff_cdeg`
2. **Projection math** (~150 lines): `distance_to_segment_squared`, `project_to_route`
3. **Search strategies** (~600 lines): 4 variants of `find_best_segment_*`, `best_eligible`, `global_search_fallback`
4. **Coordinate conversion** (~100 lines): `latlon_to_cm_absolute_with_lat_avg`, no_std helpers
5. **Tests** (~200 lines): All tests in one module

**Current Issues:**
- **Testability**: Can't unit test heading logic without full RouteData fixture
- **Cognitive load**: 1093 lines requires scrolling to understand related code
- **Coupling**: Heading and projection are tested through search, not directly

## Design Decisions

### Split Strategy

**Decision:** Extract by concern (Option 1 from brainstorming)

**Rationale:**
- Follows existing `kalman/` module pattern (`mod.rs`, `filters.rs`, `hysteresis.rs`)
- Each module has single, testable responsibility
- Clear dependency hierarchy: `search` → `heading` + `projection`
- Public API preserved through re-exports

### Module Responsibilities

**`heading.rs` (~150 lines):**
- Pure heading filter logic
- Knows about: first-fix relaxation, sentinel heading, speed-based gating
- No knowledge of: grid structure, route data, search strategies

**`projection.rs` (~200 lines):**
- Geometric calculations: distance to segment, route projection
- Pure math functions
- No knowledge of: heading, search, grid structure

**`search.rs` (~400 lines):**
- Search orchestration using `heading` and `projection`
- Implements all 4 search variants
- Knows about: grid structure, search windows, fallback strategies

**`mod.rs` (~200 lines):**
- Public API facade
- Re-exports all public functions
- Coordinate conversion and no_std helpers
- Backward compatibility

### Testing Strategy

**Decision:** Tests-first with independent module test suites

**Rationale:**
- `heading` and `projection` tests use primitive inputs only (no RouteData)
- `search` tests use RouteData fixtures (existing `create_test_route_data`)
- Each module's tests run independently
- Property-based tests for math functions (proptest)

## Architecture

### Module Structure

```
gps_processor/src/map_match/
├── mod.rs          # Public API, re-exports, coordinate conversion
├── heading.rs      # Heading filter: eligible?, threshold calculation
├── projection.rs   # Distance to segment, route projection
└── search.rs       # Search strategies: restricted, grid_only, fallbacks
```

### Dependencies

```
mod.rs → heading, projection, search
search → heading, projection
heading → (none, just shared types)
projection → (none, just shared types)
```

### Data Flow

```

┌─────────────────────────────────────────────────────────────┐
│ find_best_segment_restricted(gps_x, gps_y, heading, speed) │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
        ┌──────────────────────────────────────┐
        │ Phase 1: Window search (best_eligible)│
        └──────────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
    ┌──────────────────┐      ┌──────────────────┐
    │ heading.rs       │      │ projection.rs    │
    │ heading_eligible │      │ distance_to_seg  │
    │ (is direction    │      │ _squared         │
    │  plausible?)     │      │                  │
    └──────────────────┘      └──────────────────┘
              │                         │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │ Found eligible within   │
              │ SIGMA_GPS_CM threshold? │
              └─────────────────────────┘
                     │ YES          │ NO
                     ▼              ▼
              ┌─────────┐    ┌─────────────────────
              │ Return  │    │ Phase 2: Grid search │
              │ result  │    │ (visit 3x3 cells)
              └─────────┘    └─────────────────────┘
                                    │
                           ┌────────┴────────┐
                           ▼                 ▼
                   ┌──────────────┐  ┌──────────────┐
                   │ heading.rs   │  │ projection.r
                   │ (eligible?)  │  │ (distance)   │
                   └──────────────┘  └─────────────
                           │
                           ▼
              ┌─────────────────────────┐
              │ Return best_eligible    │
              │ or best_any (fallback)  │
              └─────────────────────────┘

```

## Components

### `heading.rs`

**Public API:**
```rust
pub fn heading_eligible(
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg_heading: HeadCdeg,
    is_first_fix: bool,
) -> bool
```

**Internal:**
```rust
fn heading_threshold_cdeg(w: i32) -> u32
fn heading_weight(v_cms: SpeedCms) -> i32
fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg
```

**Tests:**
- Sentinel heading (`i16::MIN`) always eligible
- Stopped (v=0) → gate disabled
- Moving → threshold scales with speed
- First fix → relaxed 180° threshold
- Heading difference symmetric and ≤ 180°

---

### `projection.rs`

**Public API:**
```rust
pub fn distance_to_segment_squared(
    x: DistCm,
    y: DistCm,
    seg: &RouteNode,
) -> Dist2

pub fn project_to_route(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    route_data: &RouteData,
) -> DistCm
```

**Tests:**
- Point on segment → distance = 0
- Perpendicular distance
- Clamped projection (before/after segment)
- Zero-length segment
- Projection at segment boundaries

---

### `search.rs`

**Public API:**
```rust
pub fn find_best_segment_restricted(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    last_idx: usize,
    is_first_fix: bool,
) -> (usize, i64)

pub fn find_best_segment_grid_only(...) -> (usize, i64)
pub fn find_best_segment_grid_only_with_min_max_s(...) -> (usize, i64)
pub fn find_best_segment_grid_only_with_min_s(...) -> (usize, i64)
```

**Internal:**
```rust
fn best_eligible(...) -> (usize, Dist2, bool, usize, Dist2)
fn global_search_fallback(...) -> (usize, Dist2)
```

**Tests:**
- Window early exit when within threshold
- Grid fallback when outside window
- Heading filter applied correctly
- Min/max constraints respected
- Falls back to best_any when no eligible segment

---

### `mod.rs`

**Re-exports:**
```rust
pub use heading::heading_eligible;
pub use projection::{distance_to_segment_squared, project_to_route};
pub use search::{
    find_best_segment_restricted,
    find_best_segment_grid_only,
    find_best_segment_grid_only_with_min_max_s,
    find_best_segment_grid_only_with_min_s,
};
```

**Public functions:**
```rust
pub fn latlon_to_cm_absolute_with_lat_avg(...) -> (DistCm, DistCm)
pub fn segment_score(gps_x: DistCm, gps_y: DistCm, seg: &RouteNode) -> Dist2
```

**No_std helpers:** `f64_cos`, `f64_round`, `to_radians_compat`

## Migration Path

### Phase 1: Create Modules (No Behavior Change)

1. **Create `heading.rs`**
   - Move `heading_eligible`, `heading_threshold_cdeg`, `heading_weight`, `heading_diff_cdeg`
   - Add `pub` to `heading_eligible`
   - Run: `cargo test -p gps_processor heading`
   - Expected: All existing tests pass

2. **Create `projection.rs`**
   - Move `distance_to_segment_squared`, `project_to_route`
   - Add `pub` to both
   - Run: `cargo test -p gps_processor projection`
   - Expected: All existing tests pass

3. **Create `search.rs`**
   - Move `find_best_segment_*` functions, `best_eligible`, `global_search_fallback`
   - Add `use crate::map_match::heading::heading_eligible`
   - Add `use crate::map_match::projection::distance_to_segment_squared`
   - Run: `cargo test -p gps_processor search`
   - Expected: All existing tests pass

4. **Update `mod.rs`**
   - Add `pub mod heading;`, `pub mod projection;`, `pub mod search;`
   - Add re-exports
   - Keep `segment_score`, `latlon_to_cm_absolute_with_lat_avg`
   - Keep no_std helpers
   - Run: `cargo test -p gps_processor`
   - Expected: All tests pass, no API breakage

### Phase 2: Verify Integration

1. **Run full test suite**
   ```bash
   cargo test -p gps_processor
   cargo test -p pipeline
   cargo test -p pico2-firmware
   ```
   Expected: All pass

2. **Run integration tests**
   ```bash
   make run ROUTE_NAME=ty225 SCENARIO=normal
   ```
   Expected: Same output as before

### Phase 3: Add New Tests (Opportunity)

Now that modules are isolated, add focused tests:

- `heading.rs`: First-fix edge cases, speed gating boundaries
- `projection.rs`: Property-based tests with proptest
- `search.rs`: Min/max constraint edge cases

### Rollback Plan

If any phase fails:
```bash
git checkout HEAD -- crates/pipeline/gps_processor/src/map_match/
```

Each phase is atomic and tested incrementally.

## Success Criteria

- [ ] `heading.rs` tests pass with no RouteData fixture
- [ ] `projection.rs` tests pass with minimal fixtures
- [ ] `search.rs` tests pass with RouteData fixtures
- [ ] `cargo test -p gps_processor` passes
- [ ] `cargo test -p pipeline` passes
- [ ] `cargo test -p pico2-firmware` passes
- [ ] Integration test produces identical output
- [ ] No warnings from `cargo clippy`
- [ ] Each module < 300 lines
- [ ] Public API unchanged (backward compatible)

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking existing behavior | Tests first - any change caught immediately |
| Module coupling issues | Clear dependency hierarchy; `heading` and `projection` have no dependencies |
| API breakage | Re-exports preserve existing import paths |
| Integration test differences | Phase 2 verification catches any behavior changes |

## Related Files

- `crates/pipeline/gps_processor/src/map_match.rs` — Current implementation (1093 lines)
- `crates/pipeline/gps_processor/src/kalman/` — Existing module pattern to follow
- `docs/superpowers/specs/2026-05-02-map-match-refactor-design.md` — Previous minimal refactor design

## Version History

- 2026-05-08: Initial design (comprehensive module split)
