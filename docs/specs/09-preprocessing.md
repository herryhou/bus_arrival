# Preprocessing Specification

## Overview
Phase 1: Route simplification (Douglas-Peucker) and linearization. Converts GeoJSON to binary route data.

## Invariants (MUST)

- [ ] Douglas-Peucker epsilon: 5000 cm (50 m)
- [ ] Max segment length: 10000 cm (100 m)
- [ ] Curve protection: never simplify sharp corners (< 90°)
- [ ] Linearization: cumulative distance from route start
- [ ] Precompute: segment vectors, headings, lengths

## Related Files

- `crates/preprocessor/` — Preprocessor implementation
- `docs/bus_arrival_tech_report_v8.md#4-5` — Algorithm details
