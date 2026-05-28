# Android History GPS Log Simulation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Android History GPS logs drive the normal detection pipeline as a simulation source.

**Architecture:** Add a focused GPS-log parser/timing module, store pending simulation selection in preferences, expose a simulate action from History, and add a DetectionService simulation action that feeds parsed `Location` values into `processLocation`. Reuse existing replay controls for play/pause/speed display, but keep arbitrary seek out of scope.

**Tech Stack:** Kotlin, Android foreground service, Compose, Robolectric unit tests, coroutines.

---

### Task 1: Parser And Timing

**Files:**
- Create: `android/app/src/main/java/com/busarrival/app/data/gpslog/RecordedGpsFix.kt`
- Test: `android/app/src/test/java/com/busarrival/app/data/gpslog/RecordedGpsFixTest.kt`

- [ ] Write failing tests for valid rows, malformed-row skipping, all-invalid failure, `Location` conversion, speed clamping, and delay scaling.
- [ ] Run `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.gpslog.RecordedGpsFixTest` and confirm failures are due to missing implementation.
- [ ] Implement `RecordedGpsFix`, `RecordedGpsLogParser`, `GpsSimulationTiming`, and `SupportedGpsPlaybackSpeeds`.
- [ ] Re-run the same test and confirm it passes.

### Task 2: History Handoff

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/data/preferences/DetectionPreferences.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/viewmodel/HistoryViewModel.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/history/components/GpsLogRow.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/history/HistoryScreen.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/navigation/BusArrivalNavGraph.kt`
- Test: `android/app/src/test/java/com/busarrival/app/presentation/viewmodel/HistoryViewModelTest.kt`

- [ ] Add failing tests that `requestSimulation()` writes pending reference/display name for inactive logs and rejects active logs.
- [ ] Run `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.viewmodel.HistoryViewModelTest`.
- [ ] Add pending simulation preferences and History view-model/UI simulation action.
- [ ] Re-run the History test.

### Task 3: Detection Simulation Wiring

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionService.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/viewmodel/DetectionViewModel.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt`

- [ ] Add a service simulation action that loads active route, parses the selected log, disables GPS logging, starts foreground mode, and emits parsed `Location` values through `processLocation`.
- [ ] Add a source generation guard so late emissions after stop do not process.
- [ ] Detection consumes pending simulation preferences on resume, loads log duration for controls, clears the pending request, and starts/stops simulation from play/pause.
- [ ] Existing live start stops any loaded simulation state first.

### Task 4: Verification

- [ ] Run parser and History tests.
- [ ] Run focused Detection-related tests if added.
- [ ] Run `rtk ./gradlew :app:compileDebugKotlin`.
