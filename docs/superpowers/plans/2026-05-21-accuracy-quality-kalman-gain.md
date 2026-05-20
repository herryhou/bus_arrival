# Accuracy Quality Kalman Gain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Android live detection and Rust JSONL replay prefer Android accuracy bins for Kalman position gain while preserving true NMEA HDOP fallback.

**Architecture:** Add an `AccuracyQuality` mapping in Kotlin and Rust with the same bins and gain values. Kalman gain selection becomes `accuracy -> hdop -> POOR`, while `hdop_x10` remains reserved for true NMEA HDOP.

**Tech Stack:** Kotlin/JVM unit tests, Android `Location`, Rust workspace crates (`shared`, `pipeline`, `gps_processor`, `pico2-firmware`), fixed-point integer units.

---

## File Structure

- Modify: `android/app/src/main/java/com/busarrival/app/data/pipeline/types/SemanticTypes.kt`
  - Add `AccuracyQuality`.
- Modify: `android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt`
  - Add `accuracyM: Float?` and populate it from `Location.hasAccuracy()`.
- Modify: `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilter.kt`
  - Add `accuracyM` input and precedence logic.
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`
  - Pass `gps.accuracyM` to Kalman.
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionService.kt`
  - Pass `gps.accuracyM` to Kalman.
- Create: `android/app/src/test/java/com/busarrival/app/data/pipeline/types/AccuracyQualityTest.kt`
  - Boundary tests for Kotlin mapping.
- Create: `android/app/src/test/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilterQualityTest.kt`
  - Precedence and fallback tests for Kotlin Kalman.
- Modify: `crates/shared/src/lib.rs`
  - Add `accuracy_cm: Option<DistCm>` to `GpsPoint`.
  - Add Rust accuracy gain helper and update adaptive Kalman signature.
- Modify: `crates/pipeline/src/jsonl_reader.rs`
  - Store JSONL `a` as `accuracy_cm`, not synthetic `hdop_x10`.
- Modify: `crates/pipeline/gps_processor/src/kalman/mod.rs`
  - Pass `gps.accuracy_cm` and `gps.hdop_x10` to adaptive Kalman.
- Modify: `crates/pipeline/gps_processor/src/accumulator.rs`
  - Set `accuracy_cm: None` for NMEA-derived `GpsPoint`.
- Modify: `crates/pico2-firmware/src/estimation/kalman.rs`
  - Add same precedence-capable signature for firmware estimation Kalman.
- Modify: `crates/pico2-firmware/src/estimation/mod.rs`
  - Pass `input.gps.accuracy_cm` and `input.gps.hdop_x10`.
- Modify: `crates/pipeline/tests/jsonl_reader.rs`
  - Assert JSONL accuracy preservation.
- Modify: `crates/pipeline/gps_processor/debug_test.rs`
  - Add `accuracy_cm: None` to test `GpsPoint` literals.
- Modify: `crates/pipeline/gps_processor/tests/test_off_route_detection.rs`
  - Add `accuracy_cm: None` to test `GpsPoint` literals.
- Modify: `crates/pipeline/tests/scenarios/snap_recovery_coordination.rs`
  - Add `accuracy_cm: None` to test `GpsPoint` literals.
- Modify: `crates/pico2-firmware/tests/test_new_architecture_integration.rs`
  - Add `accuracy_cm: None` to test `GpsPoint` literals.

---

### Task 1: Kotlin AccuracyQuality Contract

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/data/pipeline/types/SemanticTypes.kt`
- Create: `android/app/src/test/java/com/busarrival/app/data/pipeline/types/AccuracyQualityTest.kt`

- [ ] **Step 1: Write Kotlin boundary tests**

Create `android/app/src/test/java/com/busarrival/app/data/pipeline/types/AccuracyQualityTest.kt`:

```kotlin
package com.busarrival.app.data.pipeline.types

import kotlin.test.Test
import kotlin.test.assertEquals

class AccuracyQualityTest {
    @Test
    fun `accuracy quality uses approved brutal bins`() {
        assertEquals(AccuracyQuality.EXCELLENT, AccuracyQuality.fromAccuracyMeters(7.99f))
        assertEquals(AccuracyQuality.GOOD, AccuracyQuality.fromAccuracyMeters(8.0f))
        assertEquals(AccuracyQuality.GOOD, AccuracyQuality.fromAccuracyMeters(20.0f))
        assertEquals(AccuracyQuality.FAIR, AccuracyQuality.fromAccuracyMeters(20.01f))
        assertEquals(AccuracyQuality.FAIR, AccuracyQuality.fromAccuracyMeters(50.0f))
        assertEquals(AccuracyQuality.POOR, AccuracyQuality.fromAccuracyMeters(50.01f))
    }

    @Test
    fun `accuracy quality uses existing fixed point gains`() {
        assertEquals(77, AccuracyQuality.EXCELLENT.ks)
        assertEquals(51, AccuracyQuality.GOOD.ks)
        assertEquals(26, AccuracyQuality.FAIR.ks)
        assertEquals(13, AccuracyQuality.POOR.ks)
        assertEquals(77, AccuracyQuality.EXCELLENT.kv)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.pipeline.types.AccuracyQualityTest
```

Expected: FAIL because `AccuracyQuality` is unresolved.

- [ ] **Step 3: Add `AccuracyQuality`**

In `android/app/src/main/java/com/busarrival/app/data/pipeline/types/SemanticTypes.kt`, add this enum immediately after `HdopQuality`:

```kotlin
// Kalman gains (Android accuracy-adaptive, in meters)
enum class AccuracyQuality(val ks: Int, val kv: Int = 77) {
    EXCELLENT(77),   // < 8m accuracy
    GOOD(51),        // 8m - 20m accuracy
    FAIR(26),        // >20m - 50m accuracy
    POOR(13)         // >50m accuracy or missing quality fallback
    ;

    companion object {
        fun fromAccuracyMeters(accuracyM: Float): AccuracyQuality {
            return when {
                accuracyM < 8.0f -> EXCELLENT
                accuracyM <= 20.0f -> GOOD
                accuracyM <= 50.0f -> FAIR
                else -> POOR
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.pipeline.types.AccuracyQualityTest
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add android/app/src/main/java/com/busarrival/app/data/pipeline/types/SemanticTypes.kt android/app/src/test/java/com/busarrival/app/data/pipeline/types/AccuracyQualityTest.kt
rtk git commit -m "Add Kotlin accuracy quality mapping"
```

---

### Task 2: Kotlin Kalman Precedence and Android Call Sites

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilter.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionService.kt`
- Create: `android/app/src/test/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilterQualityTest.kt`

- [ ] **Step 1: Write Kotlin Kalman precedence tests**

Create `android/app/src/test/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilterQualityTest.kt`:

```kotlin
package com.busarrival.app.data.pipeline.localization.kalman

import com.busarrival.app.domain.model.KalmanState
import kotlin.test.Test
import kotlin.test.assertEquals

class KalmanFilterQualityTest {
    @Test
    fun `accuracy quality wins over hdop quality`() {
        val state = KalmanState(sCm = 10_000, vCms = 0, lastSegIdx = 0)

        KalmanFilter.update(
            state = state,
            zCm = 11_000,
            vGpsCms = 0,
            accuracyM = 60.0f,
            hdopX10 = 10,
            isSoftResync = false
        )

        assertEquals(10_050, state.sCm)
    }

    @Test
    fun `hdop quality is fallback when accuracy is missing`() {
        val state = KalmanState(sCm = 10_000, vCms = 0, lastSegIdx = 0)

        KalmanFilter.update(
            state = state,
            zCm = 11_000,
            vGpsCms = 0,
            accuracyM = null,
            hdopX10 = 10,
            isSoftResync = false
        )

        assertEquals(10_300, state.sCm)
    }

    @Test
    fun `missing quality falls back to poor gain`() {
        val state = KalmanState(sCm = 10_000, vCms = 0, lastSegIdx = 0)

        KalmanFilter.update(
            state = state,
            zCm = 11_000,
            vGpsCms = 0,
            accuracyM = null,
            hdopX10 = null,
            isSoftResync = false
        )

        assertEquals(10_050, state.sCm)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.pipeline.localization.kalman.KalmanFilterQualityTest
```

Expected: FAIL because `KalmanFilter.update` has no `accuracyM` parameter.

- [ ] **Step 3: Add accuracy to Android `GpsPoint`**

In `android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt`, change the data class fields to include `accuracyM`:

```kotlin
data class GpsPoint(
    val timestamp: TimestampMs,
    val lat: Double,
    val lon: Double,
    val headingCdeg: HeadCdeg?,
    val speedCms: SpeedCms?,
    val accuracyM: Float?,
    val hdop: Float?,
    val hasFix: Boolean
)
```

In `fromLocation`, set `accuracyM` before `hdop`:

```kotlin
accuracyM = if (location.hasAccuracy()) location.accuracy else null,
hdop = null,
```

- [ ] **Step 4: Update `KalmanFilter.update` signature and gain selection**

In `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilter.kt`, change the signature:

```kotlin
fun update(
    state: KalmanState,
    zCm: DistCm,
    vGpsCms: SpeedCms,
    accuracyM: Float?,
    hdopX10: Int?,
    isSoftResync: Boolean = false
): PositionSignals {
```

Replace the standard update gain selection with:

```kotlin
val ks = when {
    accuracyM != null -> AccuracyQuality.fromAccuracyMeters(accuracyM).ks
    hdopX10 != null -> HdopQuality.fromHdop(hdopX10 / 10f).ks
    else -> AccuracyQuality.POOR.ks
}

state.sCm = sPred + (ks * (zCm - sPred)) / 256
state.vCms = (vPred + (Kv_STANDARD * (vGpsCms - vPred)) / 256).coerceAtLeast(0)
```

- [ ] **Step 5: Update Android call sites**

In `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`, update the Kalman call:

```kotlin
val signals = KalmanFilter.update(
    state = kalmanState!!,
    zCm = sCm,
    vGpsCms = gps.speedCms ?: 0,
    accuracyM = gps.accuracyM,
    hdopX10 = null,
    isSoftResync = jumpDetected
)
```

In `android/app/src/main/java/com/busarrival/app/service/DetectionService.kt`, update the Kalman call:

```kotlin
val signals = KalmanFilter.update(
    state = kalmanState!!,
    zCm = zCm,
    vGpsCms = gps.speedCms ?: 0,
    accuracyM = gps.accuracyM,
    hdopX10 = null,
    isSoftResync = false
)
```

- [ ] **Step 6: Run Kotlin tests**

Run:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.pipeline.localization.kalman.KalmanFilterQualityTest
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.pipeline.types.AccuracyQualityTest
```

Expected: PASS.

- [ ] **Step 7: Compile Android debug Kotlin**

Run:

```bash
rtk ./gradlew :app:compileDebugKotlin
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
rtk git add android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt android/app/src/main/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilter.kt android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt android/app/src/main/java/com/busarrival/app/service/DetectionService.kt android/app/src/test/java/com/busarrival/app/data/pipeline/localization/kalman/KalmanFilterQualityTest.kt
rtk git commit -m "Use Android accuracy for Kalman gain"
```

---

### Task 3: Rust Shared Accuracy Contract

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Add failing Rust tests for accuracy bins and precedence**

In `crates/shared/src/lib.rs`, add these tests inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_accuracy_quality_gain_boundaries() {
    assert_eq!(KalmanState::ks_from_accuracy_cm(799), 77);
    assert_eq!(KalmanState::ks_from_accuracy_cm(800), 51);
    assert_eq!(KalmanState::ks_from_accuracy_cm(2000), 51);
    assert_eq!(KalmanState::ks_from_accuracy_cm(2001), 26);
    assert_eq!(KalmanState::ks_from_accuracy_cm(5000), 26);
    assert_eq!(KalmanState::ks_from_accuracy_cm(5001), 13);
}

#[test]
fn test_kalman_accuracy_takes_precedence_over_hdop() {
    let mut state = KalmanState::init(10_000, 0, 0);

    state.update_adaptive(11_000, 0, Some(6000), Some(10));

    assert_eq!(state.s_cm, 10_050);
}

#[test]
fn test_kalman_hdop_fallback_when_accuracy_missing() {
    let mut state = KalmanState::init(10_000, 0, 0);

    state.update_adaptive(11_000, 0, None, Some(10));

    assert_eq!(state.s_cm, 10_300);
}

#[test]
fn test_kalman_missing_quality_uses_poor_gain() {
    let mut state = KalmanState::init(10_000, 0, 0);

    state.update_adaptive(11_000, 0, None, None);

    assert_eq!(state.s_cm, 10_050);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test -p shared accuracy_quality
rtk cargo test -p shared kalman_accuracy
rtk cargo test -p shared kalman_hdop_fallback
rtk cargo test -p shared missing_quality
```

Expected: FAIL because `ks_from_accuracy_cm` and the new `update_adaptive` signature do not exist.

- [ ] **Step 3: Add `accuracy_cm` to `GpsPoint`**

In `crates/shared/src/lib.rs`, update `GpsPoint`:

```rust
pub struct GpsPoint {
    /// Timestamp in milliseconds since epoch.
    pub timestamp: TimestampMs,
    pub lat: f64,
    pub lon: f64,
    pub heading_cdeg: Option<HeadCdeg>,
    pub speed_cms: Option<SpeedCms>,
    /// Android horizontal accuracy radius in centimeters.
    pub accuracy_cm: Option<DistCm>,
    /// True NMEA HDOP * 10 (e.g., 15 = 1.5).
    pub hdop_x10: Option<u16>,
    pub has_fix: bool,
}
```

Update `GpsPoint::new()`:

```rust
accuracy_cm: None,
hdop_x10: None,
```

- [ ] **Step 4: Update shared Kalman quality helpers**

In `impl KalmanState`, replace `update_adaptive` and add the helper:

```rust
pub fn update_adaptive(
    &mut self,
    z_cm: DistCm,
    v_gps_cms: SpeedCms,
    accuracy_cm: Option<DistCm>,
    hdop_x10: Option<u16>,
) {
    let ks = Self::ks_from_quality(accuracy_cm, hdop_x10);
    let s_pred = self.s_cm + self.v_cms;
    let v_pred = self.v_cms;
    self.s_cm = s_pred + (ks * (z_cm - s_pred)) / 256;
    self.v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0);
}

fn ks_from_quality(accuracy_cm: Option<DistCm>, hdop_x10: Option<u16>) -> i32 {
    if let Some(accuracy_cm) = accuracy_cm {
        Self::ks_from_accuracy_cm(accuracy_cm)
    } else if let Some(hdop_x10) = hdop_x10 {
        Self::ks_from_hdop(hdop_x10)
    } else {
        13
    }
}

fn ks_from_accuracy_cm(accuracy_cm: DistCm) -> i32 {
    match accuracy_cm {
        i32::MIN..=799 => 77,
        800..=2000 => 51,
        2001..=5000 => 26,
        _ => 13,
    }
}
```

Keep existing `ks_from_hdop` unchanged:

```rust
fn ks_from_hdop(hdop_x10: u16) -> i32 {
    match hdop_x10 {
        0..=20 => 77,
        21..=30 => 51,
        31..=50 => 26,
        _ => 13,
    }
}
```

- [ ] **Step 5: Update existing shared tests for signature change**

In existing tests in `crates/shared/src/lib.rs`, change calls like:

```rust
state.update_adaptive(10100, -1000, hdop);
```

to:

```rust
state.update_adaptive(10100, -1000, None, Some(hdop));
```

- [ ] **Step 6: Run shared tests**

Run:

```bash
rtk cargo test -p shared
```

Expected: PASS for `shared`; other crates may still fail to compile until Task 4 updates call sites.

- [ ] **Step 7: Commit**

Run:

```bash
rtk git add crates/shared/src/lib.rs
rtk git commit -m "Add Rust accuracy quality mapping"
```

---

### Task 4: Rust JSONL Replay and Kalman Call Sites

**Files:**
- Modify: `crates/pipeline/src/jsonl_reader.rs`
- Modify: `crates/pipeline/tests/jsonl_reader.rs`
- Modify: `crates/pipeline/gps_processor/src/kalman/mod.rs`
- Modify: `crates/pipeline/gps_processor/src/accumulator.rs`
- Modify: `crates/pipeline/gps_processor/debug_test.rs`
- Modify: `crates/pipeline/gps_processor/tests/test_off_route_detection.rs`
- Modify: `crates/pipeline/tests/scenarios/snap_recovery_coordination.rs`
- Modify: `crates/pico2-firmware/src/estimation/kalman.rs`
- Modify: `crates/pico2-firmware/src/estimation/mod.rs`
- Modify: `crates/pico2-firmware/tests/test_new_architecture_integration.rs`

- [ ] **Step 1: Add failing JSONL tests**

In `crates/pipeline/tests/jsonl_reader.rs`, extend `jsonl_minimal_record_keeps_fix`:

```rust
assert_eq!(record.gps.accuracy_cm, None);
```

Add this test:

```rust
#[test]
fn jsonl_accuracy_is_preserved_without_synthetic_hdop() {
    let mut reader = JsonReader::new();

    let record = reader
        .parse_line(r#"{"t":1779172271904,"lat":24.156562,"lon":120.649046,"a":15.952}"#)
        .expect("expected valid JSONL record");

    assert_eq!(record.gps.accuracy_cm, Some(1595));
    assert_eq!(record.gps.hdop_x10, None);
}
```

- [ ] **Step 2: Run JSONL tests to verify failure**

Run:

```bash
rtk cargo test -p pipeline --test jsonl_reader
```

Expected: FAIL because JSONL still maps `a` to `hdop_x10` or because call sites have not been updated.

- [ ] **Step 3: Update JSONL reader**

In `crates/pipeline/src/jsonl_reader.rs`, change imports:

```rust
use shared::{DistCm, GpsPoint, HeadCdeg, SpeedCms};
```

In `parse_line`, replace:

```rust
let hdop_x10 = sample.a.map(json_accuracy_to_hdop_x10);
```

with:

```rust
let accuracy_cm = sample.a.map(json_accuracy_to_cm);
let hdop_x10 = None;
```

In `GpsPoint { ... }`, add:

```rust
accuracy_cm,
hdop_x10,
```

Replace `json_accuracy_to_hdop_x10` with:

```rust
#[cfg(feature = "std")]
fn json_accuracy_to_cm(accuracy_m: f64) -> DistCm {
    (accuracy_m * 100.0) as DistCm
}
```

- [ ] **Step 4: Update gps_processor Kalman call**

In `crates/pipeline/gps_processor/src/kalman/mod.rs`, replace:

```rust
state.update_adaptive(z_raw, gps.speed_cms.unwrap_or(0), gps.hdop_x10.unwrap_or(9990));
```

with:

```rust
state.update_adaptive(
    z_raw,
    gps.speed_cms.unwrap_or(0),
    gps.accuracy_cm,
    gps.hdop_x10,
);
```

- [ ] **Step 5: Update NMEA accumulator**

In `crates/pipeline/gps_processor/src/accumulator.rs`, add `accuracy_cm: None` to the `GpsPoint` built from NMEA:

```rust
GpsPoint {
    timestamp: self.timestamp.unwrap_or(0) * 1000,
    lat: self.lat.unwrap_or(0.0),
    lon: self.lon.unwrap_or(0.0),
    heading_cdeg: self.heading,
    speed_cms: self.speed,
    accuracy_cm: None,
    hdop_x10: self.hdop,
    has_fix: self.has_fix,
}
```

- [ ] **Step 6: Update firmware estimation Kalman signature**

In `crates/pico2-firmware/src/estimation/kalman.rs`, change `update_adaptive` to:

```rust
pub fn update_adaptive(
    &mut self,
    z_raw: DistCm,
    v_gps: SpeedCms,
    accuracy_cm: Option<DistCm>,
    hdop_x10: Option<u16>,
) {
    let k_pos = if let Some(accuracy_cm) = accuracy_cm {
        if accuracy_cm < 800 {
            77
        } else if accuracy_cm <= 2000 {
            51
        } else if accuracy_cm <= 5000 {
            26
        } else {
            13
        }
    } else if let Some(hdop_x10) = hdop_x10 {
        if hdop_x10 <= 20 {
            77
        } else if hdop_x10 <= 30 {
            51
        } else if hdop_x10 <= 50 {
            26
        } else {
            13
        }
    } else {
        13
    };

    self.s_cm = self.s_cm + k_pos * (z_raw - self.s_cm) / 256;
    self.v_cms = self.v_cms + 77 * (v_gps - self.v_cms) / 256;
    self.v_cms = self.v_cms.max(0);
}
```

- [ ] **Step 7: Update firmware estimation caller**

In `crates/pico2-firmware/src/estimation/mod.rs`, replace:

```rust
let hdop_x10 = input.gps.hdop_x10.unwrap_or(9990);
let speed_cms = input.gps.speed_cms.unwrap_or(0);
state.kalman.update_adaptive(z_raw, speed_cms, hdop_x10);
```

with:

```rust
let speed_cms = input.gps.speed_cms.unwrap_or(0);
state.kalman.update_adaptive(
    z_raw,
    speed_cms,
    input.gps.accuracy_cm,
    input.gps.hdop_x10,
);
```

Keep `calculate_confidence(input.gps.hdop_x10.unwrap_or(9990), ...)` unchanged, because that confidence path is still HDOP-based and the approved scope is Kalman gain selection.

- [ ] **Step 8: Update Rust `GpsPoint` literals**

In these files, add `accuracy_cm: None` to every `GpsPoint { ... }` literal that does not intentionally test Android accuracy:

- `crates/pipeline/gps_processor/debug_test.rs`
- `crates/pipeline/gps_processor/tests/test_off_route_detection.rs`
- `crates/pipeline/tests/scenarios/snap_recovery_coordination.rs`
- `crates/pico2-firmware/tests/test_new_architecture_integration.rs`

Use this field placement near `speed_cms` and `hdop_x10`:

```rust
accuracy_cm: None,
```

- [ ] **Step 9: Run Rust tests**

Run:

```bash
rtk cargo test -p pipeline --test jsonl_reader
rtk cargo test -p shared
rtk cargo test -p pipeline
rtk cargo test -p pico2-firmware
```

Expected: PASS.

- [ ] **Step 10: Commit**

Run:

```bash
rtk git add crates/pipeline/src/jsonl_reader.rs crates/pipeline/tests/jsonl_reader.rs crates/pipeline/gps_processor/src/kalman/mod.rs crates/pipeline/gps_processor/src/accumulator.rs crates/pipeline/gps_processor/debug_test.rs crates/pipeline/gps_processor/tests/test_off_route_detection.rs crates/pipeline/tests/scenarios/snap_recovery_coordination.rs crates/pico2-firmware/src/estimation/kalman.rs crates/pico2-firmware/src/estimation/mod.rs crates/pico2-firmware/tests/test_new_architecture_integration.rs
rtk git commit -m "Use accuracy quality in Rust Kalman paths"
```

---

### Task 5: Full Verification and Documentation Check

**Files:**
- Modify only if verification exposes a missed compile or doc mismatch in files already changed by Tasks 1-4.

- [ ] **Step 1: Run Android verification**

Run:

```bash
rtk ./gradlew testDebugUnitTest
rtk ./gradlew :app:compileDebugKotlin
```

Expected: PASS.

- [ ] **Step 2: Run Rust verification**

Run:

```bash
rtk cargo test
```

Expected: PASS.

- [ ] **Step 3: Check no synthetic accuracy-to-HDOP conversion remains**

Run:

```bash
rtk grep "accuracy_to_hdop|json_accuracy_to_hdop|accuracy_m \\* 2|hdop_x10 = sample.a" crates android docs/superpowers/specs docs/superpowers/plans
```

Expected: no matches in source files. Matches in historical specs are acceptable only if they describe the old behavior.

- [ ] **Step 4: Check changed files**

Run:

```bash
rtk git status --short
rtk git diff --stat
```

Expected: only files from this plan are modified, plus any unrelated pre-existing dirty files that were present before implementation.

- [ ] **Step 5: Commit verification fixes if any were needed**

If Step 1 or Step 2 required fixes, run:

```bash
rtk git add android/app/src/main/java android/app/src/test/java crates
rtk git commit -m "Fix accuracy quality verification issues"
```

If no fixes were needed, do not create an empty commit.

---

## Plan Self-Review

Spec coverage:

- `AccuracyQuality` in Kotlin and Rust: Tasks 1 and 3.
- Accuracy before HDOP precedence: Tasks 2 and 3.
- Conservative missing-quality fallback: Tasks 2 and 3.
- Android live detection passes accuracy: Task 2.
- JSONL replay preserves accuracy semantics: Task 4.
- `hdop_x10` retained for NMEA: Tasks 3 and 4.
- Boundary and fallback tests: Tasks 1-4.
- Verification: Task 5.

Type consistency:

- Kotlin uses `accuracyM: Float?`, matching Android `Location.accuracy`.
- Rust uses `accuracy_cm: Option<DistCm>`, preserving integer runtime semantics.
- `hdopX10` remains Kotlin `Int?`; `hdop_x10` remains Rust `Option<u16>`.

Scope:

- No threshold recalibration.
- No velocity gain changes.
- No UI changes.
- No removal of `hdop_x10`.
