# Stop Index Recovery Specification

## Overview
Recovers correct stop index after GPS jump (>200 m). Uses velocity constraint to filter impossible stops.

## Trigger Conditions

GPS jump detection triggers recovery:
- Jump distance > 200 m (20000 cm)
- Activated when position fix jumps beyond expected range

## Scoring Formula

```
score(i) = |s_i - s| + 5000 × max(0, last_index - i) + vel_penalty(i)
```

Where:
- `s_i`: Stop position along route (progress_cm)
- `s`: Current GPS position (cm)
- `last_index`: Last known stop index before GPS anomaly
- `vel_penalty`: Hard exclusion if distance > V_MAX_CMS × dt

### Velocity Penalty

Hard exclusion prevents physically impossible jumps:
```
vel_penalty(i) = ∞  if dist_to_stop > V_MAX_CMS × dt_since_last_fix
vel_penalty(i) = 0   otherwise
```

Where:
- `dist_to_stop`: Distance to stop if ahead (0 if behind)
- `V_MAX_CMS`: 1667 cm/s (60 km/h, max city bus speed)
- `dt_since_last_fix`: Seconds elapsed since last valid GPS fix

## Invariants (MUST)

- [ ] Trigger: GPS jump > 200 m (20000 cm)
- [ ] Filter: Only stops within ±200 m
- [ ] Filter: Only stops ≥ last_index - 1 (no backward jumps beyond adjacent stop)
- [ ] Velocity constraint: Exclude stops requiring > 1667 cm/s × dt
- [ ] Return lowest-scoring stop within valid range
- [ ] Return None if no valid stops found

## Implementation Details

### Search Space
- Iterate all stops in route
- Skip stops outside ±200 m range
- Skip stops < last_index - 1 (prevents large backward jumps)

### Scoring Components
1. **Distance score**: Absolute difference between GPS position and stop position
2. **Index penalty**: 5000 × backward jump distance (prevents arbitrary backward jumps)
3. **Velocity penalty**: Hard exclusion for physically unreachable stops

### Edge Cases
- dt=0 (GPS fix within same second): Only stops behind or at current position valid
- Large dt (extended outage): Velocity constraint relaxed proportionally
- All stops excluded: Return None (signals recovery failure)

## Constants

```rust
const GPS_JUMP_THRESHOLD: DistCm = 20000;    // 200 m
const V_MAX_CMS: u32 = 1667;                 // 60 km/h in cm/s
const INDEX_PENALTY: i32 = 5000;             // Backward jump penalty
```

## Related Files

- `crates/pipeline/detection/src/recovery.rs` — Implementation
