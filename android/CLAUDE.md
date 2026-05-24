@AGENTS.md

## Version History

### v9.2 (2026-05-24) - DR Outage False Arrival Protection
**Problem:** During dr_outage, stop #4 triggered "Arriving" despite being 48m away.
**Root cause:** F1/F3 features used DR position instead of raw GPS during dr_outage.
**Solution:** Neutralize F1/F3 to 128 when `gpsStatus != Valid && divergence > 5000cm`.

**Changes:**
- Added `GpsStatus` enum (Valid, DrOutage, OffRoute)
- Added `PHANTOM_DIVERGENCE_CM = 5000` constant
- Updated `ProbabilityModel` with F1/F3 neutralization logic
- Fixed `PositionSignals` to use actual `zGpsCm` in detection loop and trace writing
- Added `previousGpsStatus` field to capture GPS status before mode transitions
- Added regression test `DrOutageFalseArrivalTest`

**Parity:** Matches Rust v9.2 behavior in `crates/pipeline/detection/src/probability.rs`.

---
