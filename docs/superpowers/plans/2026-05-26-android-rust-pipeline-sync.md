# Android Pipeline 與 Rust Host Pipeline 同步計劃 (Revised)

## Context

Android 用 Kotlin 重寫了 bus arrival detection pipeline，但與 Rust baseline 有多處差異導致處理結果不一致。Rust 是 truth baseline，Android 需要對齊以確保 golden tests 通過。

**經 Rust source code 驗證的關鍵差異：**
- Logistic denominator 不同 (Android k=0.02, Rust k=0.01) - **CRITICAL 2x 差異**
- 檢測範圍不一致 (Android 所有 stop，Rust 只 active corridor)
- Gaussian LUT rounding 不同 (.toInt() vs .round())
- Dwell LUT 錯誤使用 round() 應該用 integer division
- Off-route hysteresis 語義錯誤 (freeze 時機、clear 行為)
- SuspectOffRoute 狀態未跳過 projection/Kalman
- Recovery 演算法不同 (grid + min/max 約束)
- Target 不明確：Rust host 使用 fixed weights，非 adaptive
- Rejection gates 缺少 frozen/first-fix guards
- Heading 0° 處理錯誤 (應用 hasBearing())
- Has fix 表示固定 true

## 目標

- **精度目標：** 行為一致 (可接受 ±1 差異)
- **範圍：** 完整同步 (P0 + P1 + P2)
- **驗證：** 分階段驗證 (每階段後跑 golden tests)

## 方法：結構對齊

按 Rust pipeline 結構重組 Android，保持代碼組織一致。長期維護容易，新功能移植快。

---

## 階段 P0：核心概率與檢測 (Critical)

### 修改 1.1：Active corridor filtering

**文件：** `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`

**Rust source:** `crates/pipeline/src/detection_state.rs:123-129`

**問題：** 當前對所有 stop 執行 FSM，Rust 只更新 `s_cm >= corridor_start_cm && s_cm <= corridor_end_cm` 的 stop。

**修復：**

```kotlin
// Before: line 328
for ((idx, stop) in route.stops.withIndex()) {
    val state = stopStates[idx] ?: continue
    // ...
}

// After: 過濾 active stops
val activeStops = route.stops.mapIndexedNotNull { idx, stop ->
    val state = stopStates[idx] ?: return@mapIndexedNotNull null
    val inCorridor = sCm >= stop.corridorStartCm && sCm <= stop.corridorEndCm
    val notSkipped = !state.skipOnReentry
    if (inCorridor && notSkipped) idx else null
}

for (idx in activeStops) {
    val stop = route.stops[idx]
    val state = stopStates[idx]!!
    // ...
}
```

---

### 修改 1.2：Logistic denominator CRITICAL FIX

**文件：** `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/probability/ProbabilityModel.kt`

**Rust source:** `crates/pipeline/detection/src/bin/gen_luts.rs:16`, `probability.rs:34`

**問題：** Android 使用 `exp((v - V_STOP) / 50.0)` = k=0.02，Rust 使用 k=0.01。這會導致 logistic 曲線有 2x 差異，影響所有概率計算。

**修復：**

```kotlin
// Before (WRONG):
val l = 1.0 / (1.0 + exp((v - V_STOP) / 50.0))

// After (match Rust):
val k = 0.01  // gen_luts.rs line 16
val l = 1.0 / (1.0 + exp(k * (v - V_STOP)))
```

---

### 修改 1.3：Gaussian LUT 使用 round()

**文件：** `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/probability/ProbabilityModel.kt`

**Rust source:** `crates/pipeline/detection/src/probability.rs:23`

**問題：** `.toInt()` 截斷與 Rust `.round()` 不同。

**修復：**

```kotlin
// Gaussian LUT: line 172
lut[i] = (g * 255.0).roundToInt().coerceIn(0, 255)

// Logistic LUT: line 186
lut[i] = (logistic * 255.0).roundToInt().coerceIn(0, 255)
```

---

### 修改 1.4：Dwell LUT 使用 integer division (NOT round)

**文件：** `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/probability/ProbabilityModel.kt`

**Rust source:** `crates/pipeline/detection/src/probability.rs:95`

**問題：** 原計劃建議使用 `.round()`，但 Rust 使用 INTEGER DIVISION。

**Rust code:**
```rust
let p4 = ((dwell_time_s as u32) * 255 / 10).min(255);
```

**修復：**

```kotlin
// Dwell LUT: line 199 - INTEGER DIVISION, not round
lut[i] = ((t.coerceAtMost(1.0) * 255).toInt().coerceIn(0, 255))
// 或更明確：
val dwellScaled = (t * 255).toInt().coerceIn(0, 255)
lut[i] = dwellScaled
```

**驗證：** dwell=1 應給 25，不是 26。

---

### P0 測試

```bash
./gradlew test --tests DetourScenarioGoldenTest
./gradlew test --tests DrOutageFalseArrivalTest
```

---

## 階段 P1：Off-route 與恢復邏輯 (Important)

### 修改 2.1：Hysteresis 模組 - freeze on FIRST suspect tick

**新文件：** `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/hysteresis/Hysteresis.kt`

**Rust source:** `crates/pipeline/gps_processor/src/kalman/hysteresis.rs:44-96`

**關鍵差異：**
1. **Freeze 在 FIRST suspect tick** (suspectTicks == 0)，不是 off-route confirmed
2. **Normal 時不清除 frozen_s_cm** (由 snap logic 處理)
3. **Suspect 狀態也返回** (不只是 OffRoute)

**正確實現：**

```kotlin
package com.busarrival.app.data.pipeline.detection.hysteresis

import com.busarrival.app.data.pipeline.types.*

object Hysteresis {
    const val OFF_ROUTE_D2_THRESHOLD: Dist2 = 25000000L  // 50m²
    const val OFF_ROUTE_CONFIRM_TICKS: UByte = 5u
    const val OFF_ROUTE_CLEAR_TICKS: UByte = 2u

    data class State(
        val suspectTicks: UByte = 0u,
        val clearTicks: UByte = 0u,
        val frozenSCm: DistCm? = null,
        val freezeTime: TimestampMs? = null
    )

    data class Result(
        val status: Status,
        val state: State
    )

    enum class Status { Normal, Suspect, OffRoute }

    fun update(
        matchDist2: Dist2,
        lastState: State,
        currentSCm: DistCm,
        currentTime: TimestampMs
    ): Result {
        val isOffRoute = matchDist2 > OFF_ROUTE_D2_THRESHOLD

        return when {
            isOffRoute -> {
                // CRITICAL: Freeze on FIRST suspect tick (suspectTicks == 0)
                // Rust: hysteresis.rs:52-56
                val newFrozenSCm = if (lastState.suspectTicks == 0u) currentSCm else lastState.frozenSCm
                val newFreezeTime = if (lastState.suspectTicks == 0u) currentTime else lastState.freezeTime
                
                val newSuspectTicks = lastState.suspectTicks.inc()
                val status = if (newSuspectTicks >= OFF_ROUTE_CONFIRM_TICKS) Status.OffRoute else Status.Suspect
                
                Result(status, lastState.copy(
                    suspectTicks = newSuspectTicks,
                    clearTicks = 0u,
                    frozenSCm = newFrozenSCm,
                    freezeTime = newFreezeTime
                ))
            }
            else -> {
                // Good match: increment clear counter
                // Rust: hysteresis.rs:73-94
                val newClearTicks = lastState.clearTicks.inc()
                
                if (newClearTicks >= OFF_ROUTE_CLEAR_TICKS) {
                    // CRITICAL: Return Normal but DO NOT clear frozenSCm
                    // Snap logic handles clearing after successful re-entry
                    // Rust: hysteresis.rs:82-87 (line 84 comment)
                    Result(Status.Normal, lastState.copy(
                        suspectTicks = 0u,
                        clearTicks = newClearTicks
                        // frozenSCm NOT cleared here
                    ))
                } else {
                    // Still suspect (need more good ticks to clear)
                    val isActuallySuspect = lastState.suspectTicks > 0u || lastState.frozenSCm != null
                    val status = if (isActuallySuspect) Status.Suspect else Status.Normal
                    Result(status, lastState.copy(
                        clearTicks = newClearTicks
                    ))
                }
            }
        }
    }
}
```

---

### 修改 2.2：SuspectOffRoute 跳過 projection/Kalman

**文件：** `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`

**Rust source:** `crates/pipeline/gps_processor/src/kalman/mod.rs:139-145`

**問題：** 原計劃只擋 OffRoute，Rust 的 SuspectOffRoute 也立即返回，更新 last GPS time，跳過 projection/Kalman。

**修復：**

```kotlin
// Phase 3: Map matching
val matchResult = MapMatcher.match(...)

// Phase 3.5: Hysteresis (BEFORE projection/Kalman)
val hysteresisResult = Hysteresis.update(
    matchDist2 = matchResult.dist2,
    lastState = hysteresisState,
    currentSCm = kalmanState?.sCm ?: 0,
    currentTime = gps.timestamp
)

hysteresisState = hysteresisResult.state

return when (hysteresisResult.status) {
    Hysteresis.Status.OffRoute -> {
        // Confirmed off-route: return with frozen position
        PipelineResult.Success(
            sCm = hysteresisState.frozenSCm ?: kalmanState!!.sCm,
            vCms = kalmanState!!.vCms,
            arrivals = emptyList(),
            departures = emptyList()
        )
    }
    Hysteresis.Status.Suspect -> {
        // CRITICAL: Suspect ALSO skips projection/Kalman
        // Rust: mod.rs:139-145 - updates dr.last_gps_time and returns
        lastGpsTime = gps.timestamp
        PipelineResult.Success(
            sCm = hysteresisState.frozenSCm ?: kalmanState!!.sCm,
            vCms = kalmanState!!.vCms,
            arrivals = emptyList(),
            departures = emptyList()
        )
    }
    Hysteresis.Status.Normal -> {
        // Check if just transitioned from frozen state for recovery snap
        val hadFrozenPosition = /* previous state had frozen */
        if (hadFrozenPosition && hysteresisResult.state.suspectTicks == 0u) {
            // Just cleared to Normal - do recovery snap
            handleRecovery(gpsX, gpsY)
        }
        // Fall through to Phase 4
    }
}

// Phase 4: Projection (only if Normal)
val (sCm, _) = GeoCoordinateConverter.projectToRoute(...)
```

---

### 修改 2.3：Recovery snap with grid + min/max constraints

**文件：** `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`

**Rust source:** `crates/pipeline/gps_processor/src/kalman/mod.rs:150-207`

**問題：** Recovery 需要 grid search 與 min/max constraints。只在 z_reentry >= frozen_s_cm 時 snap。

**修復：**

```kotlin
private fun handleRecovery(gpsX: Int, gpsY: Int) {
    val frozenSCm = hysteresisState.frozenSCm ?: return
    
    // Use relaxed heading grid search with min_s and max_s constraints
    // Rust: mod.rs:162-175
    val maxS = frozenSCm + 500_000  // 5km forward
    val result = MapMatcher.findBestSegmentGridOnly(
        gpsX = gpsX,
        gpsY = gpsY,
        routeData = routeData,
        minSCm = frozenSCm,
        maxSCm = maxS
    )
    
    if (result == null) {
        // Snap failed - stay frozen
        return
    }
    
    val zReentry = GeoCoordinateConverter.projectToRoute(
        gpsX, gpsY, result.segIdx, routeData
    )
    
    // CRITICAL: Only snap if re-entry is forward (no backward snaps)
    // Rust: mod.rs:182 - "if z_reentry >= frozen_s"
    if (zReentry >= frozenSCm) {
        // Safe to snap
        kalmanState.sCm = zReentry
        hysteresisState = hysteresisState.copy(frozenSCm = null)
        resetStopStatesFrom(recoverStopIndex(zReentry))
        
        // EMA blend for velocity (M3 fix)
        // Rust: mod.rs:188-189
        val vGps = gps.speedCms?.clamp(0, V_MAX_CMS) ?: 0
        kalmanState.vCms = kalmanState.vCms + 3 * (vGps - kalmanState.vCms) / 10
    }
    // If z_reentry < frozen_s_cm, fall through - position stays frozen
}
```

**Note:** 需要 `MapMatcher.findBestSegmentGridOnly()` 支援 min/max 參數。

---

### 修改 2.4：使用 fixed weights (非 adaptive)

**文件：** `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/probability/ProbabilityModel.kt`

**Rust source:** `crates/pipeline/src/detection_state.rs:144-152`

**問題：** Android 可能有 adaptive switching。Rust host 使用 fixed weights。Target 應該是 fixed weights 來匹配 host。

**Rust code:**
```rust
let probability = detection::probability::compute_arrival_probability(
    signals, v_cms, stop, stop_state.dwell_time_s, gps_status,
    gaussian_lut(), logistic_lut(),
);
```

**修復：**

```kotlin
// Fixed weights from Rust: 13, 6, 10, 3 (sum = 32)
// Rust: probability.rs:130
fun compute(
    signals: PositionSignals,
    stop: Stop,
    vCms: SpeedCms,
    dwellS: Int,
    gpsStatus: GpsStatus
): Prob8 {
    val features = computeFeatures(...)
    // Fixed weights: match Rust
    val p = (13 * features.p1.value + 6 * features.p2.value + 
             10 * features.p3.value + 3 * features.p4.value) / 32
    return Prob8(p.coerceIn(0, 255))
}
```

**Note:** 移除任何 `nextStop` 參數和 adaptive switching logic。

---

### P1 測試

```bash
./gradlew test --tests "*Scenario*Test"
./gradlew test --tests "*GoldenTest"
```

---

## 階段 P2：邊界情況與 Gates (Edge Cases)

### 修改 3.1：Rejection gates with frozen/first-fix guards

**新文件：** `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/gates/RejectionGates.kt`

**Rust source:** `crates/pipeline/gps_processor/src/kalman/mod.rs:268-292`

**問題：** 需要 rejection gates，但必須 SKIP 當 frozen 或 first fix。

**實現：**

```kotlin
// RejectionGates.kt
object RejectionGates {
    private const val V_MAX_CMS = 1667
    private const val SIGMA_GPS_CM = 2000
    private const val MAX_JUMP_FACTOR = 10

    fun checkSpeedConstraint(
        zNew: DistCm, zPrev: DistCm, dt: Int
    ): Boolean {
        val distAbs = kotlin.math.abs(zNew - zPrev)
        val maxDist = V_MAX_CMS * dt.coerceAtLeast(1) + SIGMA_GPS_CM
        return distAbs <= maxDist
    }

    fun checkMonotonic(zNew: DistCm, zPrev: DistCm): Boolean {
        return zNew >= zPrev - 5000
    }

    fun checkRouteJump(
        zNew: DistCm, zPrev: DistCm, dt: Int
    ): Boolean {
        val maxPossible = (V_MAX_CMS * dt.coerceAtLeast(1) + SIGMA_GPS_CM)
        val actual = kotlin.math.abs(zNew - zPrev)
        return actual <= MAX_JUMP_FACTOR * maxPossible
    }
}
```

**整合到 DetectionPipeline** - with guards:
```kotlin
// After projection, before Kalman
val (sCm, _) = GeoCoordinateConverter.projectToRoute(...)

val dt = ((gps.timestamp - lastGpsTime) / 1000).toInt().coerceAtLeast(1)

// CRITICAL: Skip rejection when frozen or first fix
// Rust: mod.rs:268 - "if state.frozen_s_cm.is_none()"
// Rust: mod.rs:238 - "if is_first_fix" returns before reaching checks
val isFrozen = hysteresisState.frozenSCm != null
val isFirstFix = lastGpsTime == null

if (!isFrozen && !isFirstFix) {
    if (!RejectionGates.checkSpeedConstraint(sCm, kalmanState.sCm, dt) ||
        !RejectionGates.checkMonotonic(sCm, kalmanState.sCm) ||
        !RejectionGates.checkRouteJump(sCm, kalmanState.sCm, dt)) {
        // Reject GPS, use DR
        // Rust: mod.rs:272-277 - updates dr.last_gps_time
        kalmanState.sCm += kalmanState.vCms * dt
        lastGpsTime = gps.timestamp  // CRITICAL: Update DR time
        return PipelineResult.Success(...)
    }
}
```

---

### 修改 3.2：Heading 使用 hasBearing() 處理 0°

**文件：** `android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt`

**Rust source:** `crates/shared/src/lib.rs:199`

**問題：** Android `Location.bearing` defaults to 0 when no bearing。使用 `>= 0` 會把 missing bearing 當成 valid 0°。Rust 使用 `Option<HeadCdeg>` 其中 0° 是有效的。

**修復：**

```kotlin
// Before: line 48
val headingCdeg = if (location.bearing > 0) {
    (location.bearing * 100).toInt().toShort()
} else null

// After: Use hasBearing() to properly detect missing vs 0°
val headingCdeg = if (location.hasBearing()) {
    (location.bearing * 100).toInt().toShort()
} else null
```

---

### 修改 3.3：Has fix proper detection

**文件：** `android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt`

**Rust source:** `crates/shared/src/lib.rs:203`

**問題：** 當前固定 `hasFix = true`。Rust 有顯式的 `has_fix: bool` field。

**修復：**

```kotlin
// Before: line 33
val hasFix = true

// After: Infer from Location state
val hasFix = location.latitude != 0.0 && location.longitude != 0.0 &&
    location.time > 0 && location.elapsedRealtimeNanos > 0

// Note: For explicit outage detection, need provider/status policy:
// val hasFix = when (location.provider) {
//     "gps" -> location.latitude != 0.0 && location.longitude != 0.0
//     "network" -> false  // Don't trust network location for outage
//     else -> false
// }
```

---

### P2 測試

```bash
./gradlew test --tests "*Test"
# 驗證所有場景
```

---

## 修改文件清單

### 新增文件
- `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/hysteresis/Hysteresis.kt`
- `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/gates/RejectionGates.kt`

### 修改文件
- `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`
- `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/probability/ProbabilityModel.kt`
- `android/app/src/main/java/com/busarrival/app/domain/model/GpsPoint.kt`
- `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/mapmatcher/MapMatcher.kt` (新增 grid only with min/max)

---

## 驗證計劃

### 每階段後執行

```bash
# P0 完成後
./gradlew test --tests DetourScenarioGoldenTest
./gradlew test --tests DrOutageFalseArrivalTest

# P1 完成後
./gradlew test --tests "*Scenario*Test"
./gradlew test --tests "*GoldenTest"

# P2 完成後
./gradlew test
```

### 最終驗證

1. 比對 Android 生成的 trace_v2.jsonl 與 Rust golden trace
2. 驗證 arrival 序列一致
3. 驗證 off-route episode 時長一致
4. 驗證 re-entry snap 位置一致

---

## Revision History

- **2026-05-26**: Initial plan (had multiple errors based on incorrect assumptions)
- **2026-05-26**: Revised after Rust source code verification
  - Fixed logistic denominator (k=0.01, not 0.02)
  - Fixed dwell LUT (integer division, not round)
  - Fixed hysteresis semantics (freeze on first suspect, don't clear frozen_s_cm on Normal)
  - Added SuspectOffRoute handling (skip projection/Kalman)
  - Clarified target: fixed weights (Rust host), not adaptive
  - Added rejection gate guards (frozen/first-fix)
  - Fixed heading handling (use hasBearing())
  - Fixed has_fix detection

---

## Rust Source References

Key files verified:
- `crates/pipeline/gps_processor/src/kalman/mod.rs` - Main GPS pipeline with hysteresis integration
- `crates/pipeline/gps_processor/src/kalman/hysteresis.rs` - Off-route hysteresis logic
- `crates/pipeline/detection/src/probability.rs` - Probability computation with LUTs
- `crates/pipeline/detection/src/bin/gen_luts.rs` - LUT generation (logistic k=0.01)
- `crates/pipeline/src/detection_state.rs` - Detection state machine (uses fixed weights)
- `crates/shared/src/lib.rs` - Shared types including GpsPoint, KalmanState
