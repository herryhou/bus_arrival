# DP Mapper

Globally optimal bus stop-to-segment mapping using dynamic programming.

## Overview

This crate implements a Viterbi-like DAG shortest path algorithm to map bus stops onto route segments. Unlike greedy approaches that make locally optimal decisions, the DP mapper finds the **globally optimal** mapping that minimizes total projection distance while preserving monotonic progress.

## Algorithm

1. **Spatial Grid Indexing** - O(k) segment queries using 100m × 100m cells
2. **Candidate Generation** - For each stop, project onto K nearest segments (default K=15)
3. **DP Forward Pass** - Sorted sweep finds minimum-cost path through candidate layers
4. **Backtrack Reconstruction** - Extract optimal path from DP tables

### Why DP Beats Greedy

```
Greedy (locally optimal):
Stop 1 → segment A (dist=8m)
Stop 2 → segment D (dist=80m)  ← only valid option after choosing A
Total: 88m

DP (globally optimal):
Stop 1 → segment B (dist=12m)
Stop 2 → segment C (dist=5m)
Total: 17m
```

The greedy approach is 5× worse on this example because it doesn't consider future stops.

## Usage

```rust
use dp_mapper::map_stops;

// Stop locations in centimeter coordinates
let stops_cm = vec![(0, 0), (5000, 0), (10000, 0)];

// Route nodes (linearized from GPS coordinates)
let route_nodes = vec![/* ... */];

// Map stops to route progress values
let progress_values = map_stops(&stops_cm, &route_nodes, None);

// Result: non-decreasing progress values in input order
assert!(progress_values[0] <= progress_values[1]);
assert!(progress_values[1] <= progress_values[2]);
```

## Complexity

- **Time:** O(M × K log K) where M = number of stops, K = candidates per stop
- **Space:** O(M × K)

For typical routes (M=35, K=15): < 10ms

## Public API

### `map_stops`

```rust
pub fn map_stops(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    k: Option<usize>,
) -> Vec<i32>
```

**Parameters:**
- `stops_cm` - Stop locations as (x, y) tuples in centimeters
- `route_nodes` - Linearized route with segment information
- `k` - Number of candidates per stop (None = default 15)

**Returns:**
- Progress values in input order (validated, non-decreasing)

## Module Structure

- `grid` - Spatial indexing for fast segment queries
- `candidate` - Projection and K-selection
- `pathfinding` - DP solver with forward pass and backtrack

## Testing

Run tests with:
```bash
cargo test -p dp_mapper
```

All tests pass (29 tests total):
- 15 unit tests for grid and candidate modules
- 6 tests for DP solver
- 7 integration tests with synthetic routes
- 1 doc test

## References

- Algorithm specification: `docs/stop_segment_mapping_algorithm.md`
- Design document: `docs/superpowers/specs/2026-03-18-dp-mapper-design.md`
