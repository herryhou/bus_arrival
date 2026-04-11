
## ✅ FIXED: vel_penalty Makes Recovery Return `None` for Real GPS Jumps

**Status**: Resolved in commit - added `dt_since_last_fix` parameter

The test documents it explicitly:

```rust
// Stop 1 (5000): dist 3900, vel_penalty i32::MAX (dist 3900 > 3000) -> excluded
// Stop 2 (9000): dist 7900, vel_penalty i32::MAX (dist 7900 > 3000) -> excluded
// Both stops excluded by velocity constraint (physically impossible to reach in 1s)
assert_eq!(find_stop_index(1100, 1000, 1, &stops, 2), None);
```

**Root Cause**: `vel_penalty` was comparing distance directly against `V_MAX_CMS` (3000 cm = 30m), effectively assuming a fixed 1-second interval. This excluded all stops > 30m away, even for legitimate GPS recovery scenarios where multiple seconds may have elapsed.

**Fix Applied**:
1. Added `dt_since_last_fix: u64` parameter to `find_stop_index()`
2. Changed velocity penalty calculation to: `max_reachable = V_MAX_CMS * dt_since_last_fix`
3. Now correctly excludes only stops that would require exceeding max speed given the elapsed time

**Implementation** (`crates/pipeline/detection/src/recovery.rs:30-62`):
```rust
pub fn find_stop_index(
    s_cm: DistCm,
    _v_filtered: SpeedCms,  // Reserved for future use
    dt_since_last_fix: u64,  // Seconds since last valid fix
    stops: &[Stop],
    last_index: u8,
) -> Option<usize> {
    // ...
    let max_reachable = V_MAX_CMS as u64 * dt_since_last_fix;
    let vel_penalty = if dist_to_stop > max_reachable {
        i32::MAX
    } else {
        0
    };
```

**Test Coverage**: All existing tests updated with appropriate `dt` values, plus new test `test_gps_recovery_with_realistic_elapsed_time()` demonstrating the fix.
