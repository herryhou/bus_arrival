# Accuracy Quality Kalman Gain Design

**Date:** 2026-05-21
**Status:** Design
**Scope:** Android live detection and Rust JSONL replay

## Overview

Use Android horizontal accuracy as a first-class GPS quality signal for Kalman position gain selection. The system will keep the existing `hdop_x10` path for true NMEA HDOP, but when Android accuracy is available it takes precedence over HDOP.

The goal is to make Android live detection and JSONL replay use the same quality contract instead of converting Android accuracy into synthetic HDOP.

## Motivation

Android `Location.accuracy` is a horizontal accuracy radius in meters. It is not HDOP, but the current JSONL replay path converts it to `hdop_x10` with `accuracy_m * 2`, and Android live detection currently passes no HDOP at all.

This creates inconsistent behavior:

- Android live detection falls back to a fixed default gain.
- JSONL replay uses an implied HDOP value that does not preserve the intended bins.
- NMEA processing and Android accuracy semantics are mixed in one field.

## Accuracy Quality Mapping

Add an `AccuracyQuality` enum on both Kotlin and Rust paths.

| Android accuracy | Quality | Position gain `Ks` | Rationale |
| --- | --- | ---: | --- |
| `< 8m` | `EXCELLENT` | `128` | High confidence; reduced lag. Increased from 77 after observing 5-10m lag at Ks=77. |
| `8m..20m` | `GOOD` | `51` | Matches current `SIGMA_GPS_CM` of 20m. |
| `20m..50m` | `FAIR` | `26` | Significant noise; trust prediction more. |
| `> 50m` | `POOR` | `13` | Near the off-route threshold; minimize GPS impact. |

Boundary rules are explicit:

- `accuracy < 8.0` -> `EXCELLENT`
- `accuracy <= 20.0` -> `GOOD`
- `accuracy <= 50.0` -> `FAIR`
- otherwise -> `POOR`

## Quality Precedence

Kalman gain selection must use this order:

```text
accuracy present -> AccuracyQuality.ks
else hdop_x10 present -> HdopQuality.ks
else -> AccuracyQuality.POOR.ks
```

The default for missing quality data is conservative: `POOR / 13`.

## Kotlin Design

### Types

Add `AccuracyQuality` next to `HdopQuality` in `SemanticTypes.kt`:

```kotlin
enum class AccuracyQuality(val ks: Int, val kv: Int = 77) {
    EXCELLENT(128),  // Increased from 77 to reduce Kalman lag
    GOOD(51),
    FAIR(26),
    POOR(13);

    companion object {
        fun fromAccuracyMeters(accuracyM: Float): AccuracyQuality = when {
            accuracyM < 8.0f -> EXCELLENT
            accuracyM <= 20.0f -> GOOD
            accuracyM <= 50.0f -> FAIR
            else -> POOR
        }
    }
}
```

`HdopQuality` remains unchanged and continues to represent real HDOP categories.

### Kalman Input

Update `KalmanFilter.update(...)` to accept optional accuracy:

```kotlin
fun update(
    state: KalmanState,
    zCm: DistCm,
    vGpsCms: SpeedCms,
    accuracyM: Float?,
    hdopX10: Int?,
    isSoftResync: Boolean = false
): PositionSignals
```

Gain selection:

```kotlin
val ks = when {
    accuracyM != null -> AccuracyQuality.fromAccuracyMeters(accuracyM).ks
    hdopX10 != null -> HdopQuality.fromHdop(hdopX10 / 10f).ks
    else -> AccuracyQuality.POOR.ks
}
```

Soft resync keeps its existing fixed conservative gains and does not use either quality signal.

### Android Live Detection

At Android `Location` call sites, pass:

```kotlin
val accuracyM = if (location.hasAccuracy()) location.accuracy else null
```

Keep `hdopX10 = null` for Android `Location`, because the platform API does not provide HDOP.

## Rust Design

### Shared GPS Data

Add `accuracy_cm: Option<DistCm>` to `GpsPoint`.

Rationale:

- `hdop_x10` remains true HDOP.
- JSONL replay can preserve Android accuracy semantics.
- Downstream Kalman selection can apply the same precedence as Kotlin.
- Rust runtime code keeps using integer semantic units; JSONL floating-point parsing stays at the std input boundary.

`GpsPoint::new()` defaults `accuracy_cm` to `None`.

### Accuracy Mapping

Add an `AccuracyQuality` helper near existing Kalman gain logic. It may be an enum or a compact function, but tests should assert the named quality bins.

```rust
fn ks_from_accuracy_cm(accuracy_cm: DistCm) -> i32 {
    if accuracy_cm < 800 {
        128  // Increased from 77 to reduce Kalman lag
    } else if accuracy_cm <= 2000 {
        51
    } else if accuracy_cm <= 5000 {
        26
    } else {
        13
    }
}
```

### Kalman Input

Extend adaptive update inputs to include `accuracy_cm: Option<DistCm>` where JSONL-derived data can reach Kalman:

```rust
pub fn update_adaptive(
    &mut self,
    z_cm: DistCm,
    v_gps_cms: SpeedCms,
    accuracy_cm: Option<DistCm>,
    hdop_x10: Option<u16>,
)
```

Gain selection:

```rust
let ks = if let Some(accuracy_cm) = accuracy_cm {
    Self::ks_from_accuracy_cm(accuracy_cm)
} else if let Some(hdop_x10) = hdop_x10 {
    Self::ks_from_hdop(hdop_x10)
} else {
    13
};
```

Firmware NMEA callers can pass `None` for `accuracy_cm` and retain HDOP behavior.

### JSONL Replay

Change `JsonReader` so field `a` maps to `GpsPoint.accuracy_cm`, not synthetic `hdop_x10`.

Before:

```rust
let hdop_x10 = sample.a.map(json_accuracy_to_hdop_x10);
```

After:

```rust
let accuracy_cm = sample.a.map(|a| (a * 100.0) as DistCm);
let hdop_x10 = None;
```

Remove or stop using `json_accuracy_to_hdop_x10`.

## Data Flow

Android live:

```text
Location.accuracy -> AccuracyQuality -> Kalman Ks
```

Android JSONL replay:

```text
JSONL field a -> GpsPoint.accuracy_cm -> AccuracyQuality -> Kalman Ks
```

NMEA:

```text
GGA/GSA HDOP -> GpsPoint.hdop_x10 -> HdopQuality -> Kalman Ks
```

Missing quality:

```text
no accuracy, no HDOP -> POOR -> Ks 13
```

## Compatibility

This is a behavior change for Android live detection and JSONL replay.

- Android live detection will no longer use the standard `51` gain when accuracy exists; it will choose the accuracy bin.
- Android live detection without accuracy will use `13` instead of the previous standard gain fallback.
- JSONL replay will no longer interpret `a` as synthetic HDOP.
- NMEA behavior remains unchanged when HDOP is present.
- NMEA without HDOP uses the conservative missing-quality fallback.

## Testing

### Kotlin Unit Tests

- `AccuracyQuality.fromAccuracyMeters(7.99f) == EXCELLENT`
- `AccuracyQuality.fromAccuracyMeters(8.0f) == GOOD`
- `AccuracyQuality.fromAccuracyMeters(20.0f) == GOOD`
- `AccuracyQuality.fromAccuracyMeters(50.0f) == FAIR`
- `AccuracyQuality.fromAccuracyMeters(50.01f) == POOR`
- Kalman uses accuracy before HDOP when both are passed.
- Kalman falls back to HDOP when accuracy is missing.
- Kalman falls back to `POOR / 13` when both quality signals are missing.

### Rust Unit Tests

- Same boundary tests for Rust `AccuracyQuality` or `ks_from_accuracy_cm`.
- Adaptive Kalman gain uses accuracy before HDOP.
- Adaptive Kalman gain falls back to HDOP when accuracy is missing.
- Adaptive Kalman gain falls back to `13` when both quality signals are missing.
- JSONL reader stores field `a` as `accuracy_cm`.
- JSONL reader no longer sets `hdop_x10` from `a`.
- NMEA accumulator/parser still sets `hdop_x10` as before.

### Integration Checks

- Run Android unit tests touching `KalmanFilter`.
- Run Rust tests for `shared`, `pipeline`, and JSONL reader.
- Replay a JSONL trace and confirm processing succeeds with accuracy-based gain selection.

## Out of Scope

- Recalibrating the four thresholds from field data.
- Changing velocity gain `Kv`.
- Changing off-route thresholds.
- Adding Android UI for displaying accuracy quality.
- Removing `hdop_x10`.

## Success Criteria

- Both Kotlin and Rust expose the same `AccuracyQuality` mapping.
- Accuracy is used before HDOP when available.
- HDOP remains available and unchanged for NMEA.
- Missing quality data maps to `POOR / 13`.
- JSONL replay preserves Android accuracy semantics.
- Tests cover boundaries, precedence, and fallback behavior.
