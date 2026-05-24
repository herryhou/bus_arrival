# Android DR Outage Protection

Android implementation matches Rust v9.2 behavior for preventing false arrivals during dr_outage.

## Key Files

- `SemanticTypes.kt` - GpsStatus enum, PHANTOM_DIVERGENCE_CM constant
- `ProbabilityModel.kt` - F1/F3 neutralization logic
- `DetectionPipeline.kt` - GPS status capture, integration with probability model

## Behavior

When `gpsStatus != Valid` AND `divergence > 5000cm`:
- F1 (distance likelihood) → 128 (neutral)
- F3 (progress likelihood) → 128 (neutral)

When `gpsStatus == Valid` AND `divergence > 2000cm`:
- F1 uses `s_cm` instead of `z_gps_cm` (poor map matching fallback)

This prevents false arrivals when DR position ≠ actual GPS position.

## Android Difference

Rust derives GpsStatus from GPS record status field. Android captures GPS status from Kalman output before mode transitions, because detection only runs when mode is Normal.

## Testing

Run: `./gradlew testDebugUnitTest --tests DrOutageFalseArrivalTest`

## Reference

- Design spec: `docs/superpowers/specs/2026-05-24-android-dr-outage-protection-design.md`
- Rust implementation: `crates/pipeline/detection/src/probability.rs`
