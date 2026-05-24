# Android DR Outage Protection Design

**Date:** 2026-05-24
**Status:** Approved
**Parity:** Rust v9.2

## Problem Statement

During `dr_outage` state, Android's arrival detection incorrectly triggered "Arriving" for stops that were 48m+ away from the actual GPS position. This occurred because the probability model used Dead-Reckoning (DR) extrapolated position (`sCm`) for both raw GPS distance (`zGpsCm`) and Kalman position (`sCm`) calculations.

### Root Cause

In `DetectionPipeline.kt` trace writing (line 173-176):
```kotlin
val detectionSignals = PositionSignals(
    zGpsCm = positionSignals.sCm,  // BUG: uses DR position for raw GPS
    sCm = positionSignals.sCm
)
```

F1 (distance likelihood) used `sCm` instead of actual raw GPS, causing false arrivals when:
- GPS jumped into dr_outage
- DR position drifted close to a stop
- Actual GPS position was 48m+ away

## Solution

Port Rust v9.2 fix: Neutralize F1 and F3 features to 128 (neutral) when `gpsStatus != Valid` AND `divergence > PHANTOM_DIVERGENCE_CM`.

**Critical Android Difference:** In Android, detection only runs when `modeState.mode == Mode.Normal`. By the time probability is computed, the mode has already transitioned back to Normal. Therefore, GPS status must be CAPTURED before mode transitions, not derived from current mode state.

## Design

### 1. Type System Additions

**File:** `SemanticTypes.kt`

Add `GpsStatus` enum:
```kotlin
enum class GpsStatus {
    Valid,      // Normal GPS tracking
    DrOutage,   // Dead reckoning mode
    OffRoute    // GPS diverged from route
}
```

Add constant to `PhysicalConstants`:
```kotlin
const val PHANTOM_DIVERGENCE_CM: DistCm = 5000  // 50m threshold (matches Rust)
```

**NOTE:** Do NOT add `Mode.toGpsStatus()` extension function. GPS status is captured from Kalman output, not derived from Mode state.

### 2. ProbabilityModel Changes

**File:** `ProbabilityModel.kt`

Update function signatures:
```kotlin
fun compute(
    signals: PositionSignals,
    stop: Stop,
    vCms: SpeedCms,
    dwellS: Int,
    gpsStatus: GpsStatus  // NEW
): Prob8

fun computeFeatures(
    signals: PositionSignals,
    stop: Stop,
    vCms: SpeedCms,
    dwellS: Int,
    gpsStatus: GpsStatus  // NEW
): ProbabilityFeatures
```

F1 neutralization logic:
```kotlin
private fun computeDistanceLikelihood(
    zGpsCm: DistCm,
    sCm: DistCm,
    stopProgressCm: DistCm,
    gpsStatus: GpsStatus
): Prob8 {
    val divergence = kotlin.math.abs(zGpsCm - sCm)

    if (gpsStatus != GpsStatus.Valid && divergence > PhysicalConstants.PHANTOM_DIVERGENCE_CM) {
        return Prob8(128)  // neutral
    }

    // Normal Gaussian LUT calculation
    val dCm = zGpsCm - stopProgressCm
    val absDCm = if (dCm < 0) -dCm else dCm
    val idx = (absDCm * 64 / PhysicalConstants.SIGMA_D_CM).coerceIn(0, GAUSSIAN_LUT_SIZE - 1)
    return Prob8(gaussianLut[idx])
}
```

F3 neutralization logic (similar pattern):
```kotlin
private fun computeProgressLikelihood(
    sCm: DistCm,
    zGpsCm: DistCm,
    stopProgressCm: DistCm,
    gpsStatus: GpsStatus
): Prob8 {
    val divergence = kotlin.math.abs(zGpsCm - sCm)

    if (gpsStatus != GpsStatus.Valid && divergence > PhysicalConstants.PHANTOM_DIVERGENCE_CM) {
        return Prob8(128)  // neutral
    }

    // Normal Gaussian LUT calculation
    val pCm = sCm - stopProgressCm
    val absPCm = if (pCm < 0) -pCm else pCm
    val idx = (absPCm * 64 / PhysicalConstants.SIGMA_P_CM).coerceIn(0, GAUSSIAN_LUT_SIZE - 1)
    return Prob8(gaussianLut[idx])
}
```

### 3. DetectionPipeline Integration

**File:** `DetectionPipeline.kt`

**Step 1:** Add field to track captured GPS status:
```kotlin
private var previousGpsStatus: GpsStatus = GpsStatus.Valid  // NEW: captured GPS status
```

**Step 2:** Capture GPS status BEFORE mode transitions (after Kalman update):
```kotlin
// After KalmanFilter.update() (around line 112)
val signals = KalmanFilter.update(...)

// CRITICAL: Capture GPS status BEFORE mode machine runs
val currentGpsStatus = when {
    jumpDetected || matchResult.dist2 > OFF_ROUTE_D2_THRESHOLD -> GpsStatus.OffRoute
    else -> GpsStatus.Valid
}
previousGpsStatus = currentGpsStatus

// NOW run mode machine (may transition to Normal)
val previousMode = modeState.mode
val modeUpdate = ModeMachine.update(...)
modeState = modeUpdate.state
```

**Step 3:** Fix actual detection loop (lines 311-320):
```kotlin
// BEFORE (buggy - uses sCm for both):
val detectionSignals = PositionSignals(
    zGpsCm = positionSignals.sCm,
    sCm = positionSignals.sCm
)
val probability = ProbabilityModel.compute(
    signals = detectionSignals,
    stop = stop,
    vCms = kalmanState!!.vCms,
    dwellS = state.dwellTimeS
)

// AFTER (fixed):
val detectionSignals = PositionSignals(
    zGpsCm = positionSignals.zGpsCm,  // Use actual raw GPS
    sCm = positionSignals.sCm
)
val probability = ProbabilityModel.compute(
    signals = detectionSignals,
    stop = stop,
    vCms = kalmanState!!.vCms,
    dwellS = state.dwellTimeS,
    gpsStatus = previousGpsStatus  // NEW: use captured status
)
```

**Step 4:** Also fix trace writing (lines 173-176):
```kotlin
// BEFORE (buggy):
val detectionSignals = PositionSignals(
    zGpsCm = positionSignals.sCm,
    sCm = positionSignals.sCm
)

// AFTER (fixed):
val detectionSignals = PositionSignals(
    zGpsCm = positionSignals.zGpsCm,
    sCm = positionSignals.sCm
)
```

### 4. Testing

**File:** `DrOutageFalseArrivalTest.kt` (new)

Regression test matching Rust `dr_outage_false_arrival.rs`:
```kotlin
class DrOutageFalseArrivalTest {

    @Test
    fun testDrOutageDoesNotCauseFalseArrival() {
        val routeData = loadRouteData("ty225_normal.bin")
        val pipeline = DetectionPipeline()
        pipeline.initialize(routeData)

        // Process NMEA and verify no false arrival at 80320000
        // ... implementation

        // Verify stop #4 does NOT arrive when GPS is 48m away during dr_outage
    }
}
```

### 5. Documentation

**File:** `android/CLAUDE.md`

Add version history entry:
```markdown
### v9.2 (2026-05-24) - DR Outage False Arrival Protection
- Added GpsStatus enum and PHANTOM_DIVERGENCE_CM constant
- Updated ProbabilityModel with F1/F3 neutralization
- Fixed PositionSignals to use actual zGpsCm
- Parity with Rust v9.2
```

**File:** `android/docs/android-dr-outage-protection.md` (new)

Quick reference for Android implementation.

## Data Flow

```
Location → DetectionPipeline.process()
    ↓
KalmanFilter.update() → signals (zGpsCm, sCm)
    ↓
CAPTURE currentGpsStatus (before mode transitions)
    ↓
ModeMachine.update() → ModeState (may transition to Normal)
    ↓
Detection loop (mode is now Normal, but we have captured previousGpsStatus)
    ↓
ProbabilityModel.compute(gpsStatus=previousGpsStatus, ...)
    ↓
Check: previousGpsStatus != Valid && divergence > 5000?
    ↓ YES → Return F1=128, F3=128 (neutral)
    ↓ NO → Normal Gaussian LUT calculation
    ↓
Probability computed → No false arrival
```

## Key Invariant

When `gpsStatus != Valid` AND `divergence > 5000cm`:
- F1 (distance likelihood) = 128 (neutral)
- F3 (progress likelihood) = 128 (neutral)

This prevents probability from exceeding arrival threshold (191), avoiding false arrivals.

## Files Modified

| File | Change |
|------|--------|
| `SemanticTypes.kt` | Add GpsStatus enum, PHANTOM_DIVERGENCE_CM = 5000 |
| `ProbabilityModel.kt` | Add gpsStatus param, F1/F3 neutralization logic |
| `DetectionPipeline.kt` | Add previousGpsStatus field, capture GPS status before mode transitions, fix zGpsCm bug in detection loop AND trace writing |
| `DrOutageFalseArrivalTest.kt` | New regression test |
| `android/CLAUDE.md` | Add v9.2 version history |
| `android/docs/android-dr-outage-protection.md` | New quick reference |

## Rust Parity

This design achieves full parity with Rust v9.2 implementation in `crates/pipeline/detection/src/probability.rs`:

- Same enum values (Valid, DrOutage, OffRoute)
- Same threshold (PHANTOM_DIVERGENCE_CM = 5000)
- Same neutralization logic (return 128)
- Same divergence calculation (|zGpsCm - sCm|)

**Android Difference:** Rust derives GpsStatus from GPS record status field. Android captures GPS status from Kalman output before mode transitions, because detection only runs when mode is Normal.

## Success Criteria

- ✅ No false arrivals when GPS is 48m+ away during dr_outage
- ✅ Normal arrival detection unaffected
- ✅ Test passes: `DrOutageFalseArrivalTest`
- ✅ Parity with Rust v9.2 behavior
