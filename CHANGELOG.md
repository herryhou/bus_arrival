# Changelog

## [v8.9] - 2026-04-10

### Critical Fixes
- Fix UART blocking loop - add 5-second timeout to prevent hang on GPS disconnect
- Fix u32 wrap bug in map matching when GPS outside route bounding box

### Spec Compliance
- Implement v8.4 corridor entry announcement (should_announce, called after FSM update)
- Add 3-second Kalman warmup period before arrival detection (resets on GPS outage)
- Add velocity-based hard exclusion to recovery scoring (Section 15.2)

### Code Quality
- Extract duplicate probability calculations to shared helper
- Add GGA sentence heading sentinel value (i16::MIN) with segment_score guard
- Normalize DR speed decay by dt using lookup table

### Infrastructure
- Add shared probability constants (prevent formula divergence)
- Generate LUTs from pipeline source at build time (OUT_DIR, not src/)
- Add scenario tests for all critical fixes

### Breaking Changes
- `ArrivalEvent` now includes `event_type` field
- UART changed from blocking to async
- JSON output format includes 'type' field

### Known Acceptances
- XIP misaligned memory leak: Bounded impact, firmware fails fast correctly
- DR filtered velocity: Uses direct assignment from Kalman (already smoothed)

## [v8.8] - Previous Release
- Initial implementation
