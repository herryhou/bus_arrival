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
const val PHANTOM_DIVERGENCE_CM: DistCm = 1500  // 15m threshold
```

Add extension function:
```kotlin
fun Mode.toGpsStatus(): GpsStatus = when (this) {
    Mode.Normal -> GpsStatus.Valid
    Mode.OffRoute -> GpsStatus.OffRoute
    Mode.Recovering -> GpsStatus.DrOutage
}
```

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

Derive GpsStatus from mode:
```kotlin
val gpsStatus = modeState.mode.toGpsStatus()
```

Pass to ProbabilityModel:
```kotlin
val features = ProbabilityModel.computeFeatures(
    signals = detectionSignals,
    stop = stop,
    vCms = kalmanState!!.vCms,
    dwellS = state.dwellTimeS,
    gpsStatus = gpsStatus  // NEW
)
```

Fix trace writing bug:
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
ModeMachine.update() → ModeState(mode)
    ↓
mode.toGpsStatus() → GpsStatus
    ↓
ProbabilityModel.compute(gpsStatus, signals, ...)
    ↓
computeFeatures() checks: gpsStatus != Valid && divergence > 1500?
    ↓ YES → Return F1=128, F3=128 (neutral)
    ↓ NO → Normal Gaussian LUT calculation
    ↓
Probability computed → No false arrival
```

## Key Invariant

When `gpsStatus != Valid` AND `divergence > 1500cm`:
- F1 (distance likelihood) = 128 (neutral)
- F3 (progress likelihood) = 128 (neutral)

This prevents probability from exceeding arrival threshold (191), avoiding false arrivals.

## Files Modified

| File | Change |
|------|--------|
| `SemanticTypes.kt` | Add GpsStatus enum, PHANTOM_DIVERGENCE_CM, toGpsStatus() |
| `ProbabilityModel.kt` | Add gpsStatus param, F1/F3 neutralization logic |
| `DetectionPipeline.kt` | Derive GpsStatus, pass to ProbabilityModel, fix zGpsCm bug |
| `DrOutageFalseArrivalTest.kt` | New regression test |
| `android/CLAUDE.md` | Add v9.2 version history |
| `android/docs/android-dr-outage-protection.md` | New quick reference |

## Rust Parity

This design achieves full parity with Rust v9.2 implementation in `crates/pipeline/detection/src/probability.rs`:

- Same enum values (Valid, DrOutage, OffRoute)
- Same threshold (PHANTOM_DIVERGENCE_CM = 1500)
- Same neutralization logic (return 128)
- Same divergence calculation (|zGpsCm - sCm|)

## Success Criteria

- ✅ No false arrivals when GPS is 48m+ away during dr_outage
- ✅ Normal arrival detection unaffected
- ✅ Test passes: `DrOutageFalseArrivalTest`
- ✅ Parity with Rust v9.2 behavior
