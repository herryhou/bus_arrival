# GPS Log Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the History tab with a GPS log manager that lists logs from the active storage backend, supports multi-select zip sharing, and blocks deleting the active log while it is still being recorded.

**Architecture:** Keep the existing History tab shell, but change the content and view-model responsibilities from arrival history to log management. Add a small storage manager that enumerates JSONL logs from the active backend only, reads log contents, and deletes logs; add a zip archive helper for sharing selected logs as one file. The UI should present a selectable list, mark the persisted active log, and disable delete when that log is selected.

**Tech Stack:** Android Kotlin, Jetpack Compose, AndroidX ViewModel, FileProvider, SAF/file storage, kotlinx.coroutines, JUnit, Robolectric.

---

### Task 1: Add GPS log storage and archive helpers

**Files:**
- Create: `android/app/src/main/java/com/busarrival/app/data/gpslog/GpsLogStorageManager.kt`
- Create: `android/app/src/main/java/com/busarrival/app/service/GpsLogArchive.kt`
- Test: `android/app/src/test/java/com/busarrival/app/data/gpslog/GpsLogStorageManagerTest.kt`
- Test: `android/app/src/test/java/com/busarrival/app/service/GpsLogArchiveTest.kt`

- [ ] Write tests that prove file-backed logs are listed newest-first, the active reference is marked, and zip sharing contains each selected log.
- [ ] Run the targeted unit tests and confirm they fail before implementation.
- [ ] Implement the storage manager, log loading, delete guard, zip creation, and share intent helpers.
- [ ] Re-run the targeted unit tests and confirm they pass.

### Task 2: Convert the History view model to log-manager state

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/viewmodel/HistoryViewModel.kt`
- Test: `android/app/src/test/java/com/busarrival/app/presentation/viewmodel/HistoryViewModelTest.kt`

- [ ] Write a unit test that loads logs, toggles selection, selects all, shares selected logs, and blocks delete when the active log is selected.
- [ ] Run the targeted unit test and confirm it fails before implementation.
- [ ] Replace event-history state with log-manager state and wire it to the new storage/archive helpers.
- [ ] Re-run the targeted unit test and confirm it passes.

### Task 3: Rebuild the History screen as a GPS log manager

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/history/HistoryScreen.kt`
- Create: `android/app/src/main/java/com/busarrival/app/presentation/ui/history/components/GpsLogRow.kt`
- Test: `android/app/src/test/java/com/busarrival/app/presentation/ui/history/HistoryScreenTest.kt`

- [ ] Write a UI test that renders selectable rows, an active badge, and share/delete actions.
- [ ] Run the targeted UI test and confirm it fails before implementation.
- [ ] Replace the event-history list with a selectable log manager layout.
- [ ] Re-run the targeted UI test and confirm it passes.

### Task 4: Verify the Android test suite

**Files:**
- Modify: any files touched above

- [ ] Run `rtk ./gradlew testDebugUnitTest`.
- [ ] Fix any regressions introduced by the log manager work.
- [ ] Keep the change limited to the GPS log manager behavior described in the spec.
