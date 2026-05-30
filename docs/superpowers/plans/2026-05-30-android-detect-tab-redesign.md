# Android Detect Tab Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Android Detect tab outside `MapView.kt` into a clean, readable dashboard.

**Architecture:** Keep the existing `DetectionScreen` state flow and callback wiring. Redesign the lower dashboard by updating `StatusPanel`, `GpsStatusRow`, and `TimelineScrubber`; keep map rendering isolated and untouched. Add small pure formatting helpers for testable UI text and preserve existing behavior.

**Tech Stack:** Kotlin, Jetpack Compose Material 3, Android unit tests, Gradle.

---

## Files

- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/StatusPanel.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/GpsStatusRow.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt`
- Test: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/StatusPanelFormattingTest.kt`
- Do not modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`

## Task 1: Add Formatting Tests

**Files:**
- Create: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/StatusPanelFormattingTest.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/StatusPanel.kt`

- [ ] **Step 1: Write failing tests for labels used by the redesigned dashboard**

Create `StatusPanelFormattingTest.kt` with tests for active stop labels, distance formatting, speed formatting, and state display text.

- [ ] **Step 2: Run focused test and verify it fails**

Run: `JAVA_HOME=$(/usr/libexec/java_home -v 17) rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.StatusPanelFormattingTest`

Expected: FAIL because the tested helpers are not yet exposed.

- [ ] **Step 3: Add minimal formatting helpers**

Add internal pure helpers in `StatusPanel.kt`: `formatStopLabel`, `formatDistance`, `formatSpeed`, and `formatStateLabel`.

- [ ] **Step 4: Run focused test and verify it passes**

Run the same focused Gradle command.

Expected: PASS.

## Task 2: Redesign Status Dashboard

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/StatusPanel.kt`
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/GpsStatusRow.kt`

- [ ] **Step 1: Replace dense status card with dashboard surface**

Use a low-elevation `Card`, a route/status header, a prominent current-stop panel, metric tiles, a 48 dp primary Start/Stop button, secondary camera/GPS logging controls, and a cleaner recent-events list.

- [ ] **Step 2: Keep callback wiring unchanged**

Ensure `onStartStop`, `onToggleCamera`, and `onToggleGpsLogging` remain connected to the same controls.

- [ ] **Step 3: Run focused formatting test**

Run: `JAVA_HOME=$(/usr/libexec/java_home -v 17) rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.StatusPanelFormattingTest`

Expected: PASS.

## Task 3: Redesign Replay Scrubber

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt`

- [ ] **Step 1: Update replay controls to match dashboard treatment**

Use low elevation, clearer play/pause control, speed chips/buttons, camera follow toggle, readable time/progress text, and the existing slider.

- [ ] **Step 2: Preserve replay behavior**

Keep `onPlayPause`, `onSeek`, `onSpeedChange`, `onToggleCameraFollow`, and `allowSeek` behavior unchanged.

- [ ] **Step 3: Run existing timeline tests**

Run: `JAVA_HOME=$(/usr/libexec/java_home -v 17) rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.TimelineScrubberTest`

Expected: PASS.

## Task 4: Refresh Detect Screen States And Verify

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt`

- [ ] **Step 1: Refresh permission, empty route, and error surfaces**

Use consistent Material 3 spacing and action sizing while preserving permission request, empty route, and error dismissal behavior.

- [ ] **Step 2: Verify MapView is unchanged**

Run: `rtk git diff -- android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`

Expected: no output.

- [ ] **Step 3: Run Android verification**

Run: `JAVA_HOME=$(/usr/libexec/java_home -v 17) rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.StatusPanelFormattingTest --tests com.busarrival.app.presentation.ui.detection.components.TimelineScrubberTest`

Expected: PASS.

- [ ] **Step 4: Compile Android app**

Run: `JAVA_HOME=$(/usr/libexec/java_home -v 17) rtk ./gradlew :app:compileDebugKotlin`

Expected: PASS.

- [ ] **Step 5: Screenshot verification**

If an emulator or device is available, capture before/after Detect tab screenshots. If not available, record that limitation in the completion notes.
