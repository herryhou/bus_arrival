# Android Acquiring Cold Boot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Android mirror Rust cold-boot acquiring behavior before normal localization and stop detection.

**Architecture:** Add explicit cold/warm Kalman constructors, expose heading eligibility through `MapMatcher`, and add an early cold-boot branch in `DetectionPipeline`. Cold boot bypasses normal `Hysteresis.update()` and uses `KalmanState.offRouteClearTicks` until two consecutive good heading-eligible matches snap to route.

**Tech Stack:** Kotlin, Android JVM tests, Robolectric, existing trace v2 JSON model.

---

### Task 1: Sync Spec With Validated Review

**Files:**
- Modify: `docs/superpowers/specs/2026-05-27-android-acquiring-cold-boot-design.md`

- [ ] Replace `KalmanState.init()` cold-boot guidance with `coldBoot()` and `warmBoot()` constructors.
- [ ] State that cold boot uses `KalmanState.offRouteClearTicks` and bypasses `Hysteresis.update()`.
- [ ] Add a concrete `MapMatcher.checkHeadingEligible()` wrapper task.
- [ ] Clarify that `isColdBoot = false` is set immediately after snap and before success output.

### Task 2: Add Failing State Tests

**Files:**
- Create: `android/app/src/test/java/com/busarrival/app/domain/model/KalmanStateTest.kt`

- [ ] Add tests for `KalmanState.coldBoot()`, `KalmanState.warmBoot()`, and `Hysteresis.isColdStart()`.
- [ ] Run:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.domain.model.KalmanStateTest
```

Expected: compile/test failure because constructors and helper do not exist.

### Task 3: Add Failing Pipeline Tests

**Files:**
- Modify: `android/app/src/test/java/com/busarrival/app/service/DetectionPipelineTraceV2Test.kt`

- [ ] Add tests proving first cold-boot tick returns `PipelineResult.Acquiring` and writes `"acquiring"` with zero position/velocity and no stop state output.
- [ ] Add tests proving a bad match resets the acquisition counter before two good matches clear cold boot.
- [ ] Update existing trace tests that assumed first trace status was `"normal"` to expect startup acquisition.
- [ ] Run:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.service.DetectionPipelineTraceV2Test
```

Expected: compile/test failure because `PipelineResult.Acquiring` and acquisition behavior do not exist.

### Task 4: Implement State and MapMatcher Surface

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/domain/model/StateModels.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/hysteresis/Hysteresis.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/data/pipeline/localization/mapmatcher/MapMatcher.kt`

- [ ] Add `isColdBoot` to `KalmanState`.
- [ ] Add `KalmanState.coldBoot()` and `KalmanState.warmBoot(...)`.
- [ ] Keep `KalmanState.init(...)` as a compatibility alias to `warmBoot(...)` unless all call sites are updated.
- [ ] Add `Hysteresis.isColdStart(state: KalmanState)`.
- [ ] Add `MapMatcher.checkHeadingEligible(...)` and use existing private heading math.
- [ ] Run the state tests and make them pass.

### Task 5: Implement DetectionPipeline Cold Boot

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`

- [ ] Initialize/reset `kalmanState = KalmanState.coldBoot()`.
- [ ] Add `PipelineResult.Acquiring`.
- [ ] Add early cold-boot handling before normal hysteresis/projection/Kalman/detection.
- [ ] During acquiring, write trace status `"acquiring"` with `s_cm=0`, `v_cms=0`, empty corridor, and empty stop states.
- [ ] After two consecutive good matches, snap to route, clear cold boot immediately, reset counters, blend velocity, and return success without detection for that first snapped tick.
- [ ] Run the pipeline tests and make them pass.

### Task 6: Regression Verification

**Files:**
- No production changes unless failures expose scoped regressions.

- [ ] Run focused Android tests:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.domain.model.KalmanStateTest --tests com.busarrival.app.service.DetectionPipelineTraceV2Test
```

- [ ] Run existing scenario smoke tests affected by startup trace changes:

```bash
rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.scenarios.DetourScenarioGoldenTest --tests com.busarrival.app.scenarios.Tz23ScenarioTest
```

- [ ] Run formatter if Kotlin formatting changed:

```bash
rtk ./gradlew ktfmtFormat
```
