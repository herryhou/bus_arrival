# Preprocessing Specification

## Overview
Phase 1: Route simplification (Douglas-Peucker) and linearization. Converts GeoJSON to binary route data.

## Invariants (MUST)

- [ ] Douglas-Peucker epsilon: 5000 cm (50 m)
- [ ] Max segment length: 10000 cm (100 m)
- [ ] Curve protection: never simplify sharp corners (< 90°)
- [ ] Stop protection: preserve stops and neighboring route geometry
- [ ] Linearization: cumulative distance from route start
- [ ] Precompute: segment vectors, headings, lengths
- [ ] Stop projection preserves monotonic stop order; looped routes require
      sequence-aware projection, not nearest-point greedy matching
- [ ] Segment vectors must fit `i16` after simplification

## Related Files

- `crates/preprocessor/` — Preprocessor implementation
- `docs/specs/10-spatial_index.md` — Spatial grid and binary format details
