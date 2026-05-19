# GPS Log Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current History tab with a GPS log manager that lists logs from the active storage location, supports multi-select sharing as a single zip, and blocks deleting the currently active log until recording stops.

**Architecture:** The existing History tab shell stays in place, but its content and view model responsibilities shift from event history to GPS log management. A small storage layer will enumerate log files from the active storage backend only, expose metadata for the list UI, and support loading selected logs for zip export or deletion. The UI will use multi-select state in the History screen and call into a dedicated log-manager view model for selection, share, and delete actions. The currently recording log is marked active through the persisted GPS log reference already used by detection, so the manager can show it and block deletion without needing a separate live service connection.

**Tech Stack:** Android Kotlin, Jetpack Compose, AndroidX ViewModel, FileProvider, SAF/file storage, kotlinx.coroutines, JUnit/Robolectric.

---

### Task 1: Define GPS Log Storage Metadata and Operations

**Files:**
- Create: `android/app/src/main/java/com/busarrival/app/data/gpslog/GpsLogStorageManager.kt`
- Test: `android/app/src/test/java/com/busarrival/app/data/gpslog/GpsLogStorageManagerTest.kt`

- [ ] **Step 1: Write the failing test**

```kotlin
@Test
fun listLogsReturnsSortedMetadataForActiveStorage() = runTest {
    // Arrange: create two JSONL files in the configured active storage directory.
    // Act: call listLogs().
    // Assert: metadata is returned newest-first with filename, timestamp, size, and active flag.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.gpslog.GpsLogStorageManagerTest`

Expected: FAIL because `GpsLogStorageManager` does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```kotlin
data class GpsLogMetadata(
    val filename: String,
    val reference: String,
    val modifiedAtMillis: Long,
    val sizeBytes: Long,
    val isActive: Boolean
)

object GpsLogStorageManager {
    suspend fun listLogs(): List<GpsLogMetadata> = withContext(Dispatchers.IO) {
        // Resolve the active storage root, scan for JSONL files, and map each file to metadata.
        // Mark isActive=true when the file reference matches the persisted active GPS log reference.
    }

    suspend fun loadLog(reference: String): List<String> = withContext(Dispatchers.IO) {
        // Read the selected log as JSONL and return non-blank lines.
    }

    suspend fun deleteLog(reference: String): Boolean = withContext(Dispatchers.IO) {
        // Delete the selected file reference from the active storage backend.
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.data.gpslog.GpsLogStorageManagerTest`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/data/gpslog/GpsLogStorageManager.kt android/app/src/test/java/com/busarrival/app/data/gpslog/GpsLogStorageManagerTest.kt
git commit -m "feat: add gps log storage manager"
```

### Task 2: Replace History ViewModel with Log Manager State

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/viewmodel/HistoryViewModel.kt`
- Test: `android/app/src/test/java/com/busarrival/app/presentation/viewmodel/HistoryViewModelTest.kt`

- [ ] **Step 1: Write the failing test**

```kotlin
@Test
fun selectingLogsAndSharingBuildsSingleZipSelection() = runTest {
    // Arrange: view model initialized with two logs.
    // Act: select both logs and call shareSelected().
    // Assert: emitted share state contains one zip reference that includes both filenames.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.viewmodel.HistoryViewModelTest`

Expected: FAIL because the current view model only models event history.

- [ ] **Step 3: Write minimal implementation**

```kotlin
data class LogManagerItem(
    val filename: String,
    val reference: String,
    val modifiedAtMillis: Long,
    val sizeBytes: Long,
    val isActive: Boolean,
    val isSelected: Boolean = false
)

data class LogManagerUiState(
    val isLoading: Boolean = false,
    val logs: List<LogManagerItem> = emptyList(),
    val selectedCount: Int = 0,
    val canDeleteSelected: Boolean = false,
    val error: String? = null
)

class HistoryViewModel(...) {
    fun toggleSelection(reference: String)
    fun selectAll()
    fun clearSelection()
    fun refresh()
    fun shareSelected()
    fun deleteSelected()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.viewmodel.HistoryViewModelTest`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/viewmodel/HistoryViewModel.kt android/app/src/test/java/com/busarrival/app/presentation/viewmodel/HistoryViewModelTest.kt
git commit -m "feat: add gps log manager state"
```

### Task 3: Rebuild History Screen as GPS Log Manager

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/history/HistoryScreen.kt`
- Create: `android/app/src/main/java/com/busarrival/app/presentation/ui/history/components/GpsLogRow.kt`
- Test: `android/app/src/test/java/com/busarrival/app/presentation/ui/history/HistoryScreenTest.kt`

- [ ] **Step 1: Write the failing test**

```kotlin
@Test
fun historyScreenShowsMultiSelectLogManagerControls() {
    // Arrange: ui state with two logs, one active.
    // Act: render HistoryScreen.
    // Assert: rows show checkboxes, Share button, Delete button, and active log badge.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.history.HistoryScreenTest`

Expected: FAIL because the current screen shows event history, not logs.

- [ ] **Step 3: Write minimal implementation**

```kotlin
@Composable
fun HistoryScreen(viewModel: HistoryViewModel = viewModel(...)) {
    val uiState by viewModel.uiState.collectAsState()
    // Render loading, empty state, and a selectable list of log rows.
    // Add top actions for Select all, Share, Delete.
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.history.HistoryScreenTest`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/history/HistoryScreen.kt android/app/src/main/java/com/busarrival/app/presentation/ui/history/components/GpsLogRow.kt android/app/src/test/java/com/busarrival/app/presentation/ui/history/HistoryScreenTest.kt
git commit -m "feat: turn history tab into gps log manager"
```

### Task 4: Add Zip Sharing and Delete Guard

**Files:**
- Create: `android/app/src/main/java/com/busarrival/app/service/GpsLogArchive.kt`
- Modify: `android/app/src/main/AndroidManifest.xml`
- Create: `android/app/src/main/res/xml/file_paths.xml`
- Test: `android/app/src/test/java/com/busarrival/app/service/GpsLogArchiveTest.kt`

- [ ] **Step 1: Write the failing test**

```kotlin
@Test
fun archiveBundlesMultipleLogsIntoSingleZip() = runTest {
    // Arrange: two sample log files.
    // Act: create archive for share.
    // Assert: output zip exists and contains both filenames.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.service.GpsLogArchiveTest`

Expected: FAIL because zip export does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```kotlin
object GpsLogArchive {
    fun createZip(context: Context, selected: List<GpsLogMetadata>): File
    fun shareZip(context: Context, zipFile: File): Intent
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.service.GpsLogArchiveTest`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/service/GpsLogArchive.kt android/app/src/main/AndroidManifest.xml android/app/src/main/res/xml/file_paths.xml android/app/src/test/java/com/busarrival/app/service/GpsLogArchiveTest.kt
git commit -m "feat: add zip sharing for gps logs"
```

### Task 5: Cleanup and Verification

**Files:**
- Modify: any files touched in Tasks 1-5

- [ ] **Step 1: Run the full Android unit test suite**

Run: `rtk ./gradlew testDebugUnitTest`

Expected: BUILD SUCCESSFUL.

- [ ] **Step 2: Review the log manager flow in the app**

Run the Android app and confirm:
- the History tab now shows GPS logs, not arrival events
- multi-select works
- Share exports one zip file containing the selected logs
- Delete is disabled or blocked when the active log is selected

- [ ] **Step 3: Commit cleanup**

```bash
git add .
git commit -m "feat: complete gps log manager"
```
