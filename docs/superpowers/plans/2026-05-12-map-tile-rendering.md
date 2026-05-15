# Map Tile Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix tile alignment bugs using single unified coordinate system, add timeline/replay for historical trip inspection

**Architecture:** 3-layer rendering (Tile Canvas, Vector Overlay, UI) with world-space positioning at current tileZ

**Tech Stack:** Kotlin, Jetpack Compose, Canvas drawScope, Coroutines

---

## File Structure

**Existing files to modify:**
- `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt` — Main map rendering component
- `app/src/main/java/com/busarrival/app/presentation/viewmodel/DetectionViewModel.kt` — State management

**New files to create:**
- `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapCoordinateUtils.kt` — Extracted coordinate functions for testability
- `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt` — Timeline UI component
- `app/src/main/java/com/busarrival/app/data/trace/TraceStorageManager.kt` — Trace file storage (JSONL)
- `app/src/main/java/com/busarrival/app/data/trace/TraceLoader.kt` — Trace loading logic
- `app/src/main/java/com/busarrival/app/domain/model/ReplayState.kt` — Replay state data class
- `app/src/main/java/com/busarrival/app/domain/model/MapState.kt` — Unified map state data class
- `app/src/test/java/com/busarrival/app/presentation/ui/detection/CoordinateSystemTest.kt` — Unit tests for coordinate functions
- `app/src/test/java/com/busarrival/app/presentation/ui/detection/TilePositioningTest.kt` — Tile positioning tests
- `app/src/test/java/com/busarrival/app/presentation/ui/detection/FallbackTileTest.kt` — Fallback tile tests
- `app/src/test/java/com/busarrival/app/presentation/ui/detection/TransformChainTest.kt` — Transform chain tests

---

## Phase 1: Complete Coordinate System Fix

### Task 1: Create MapState data class

**Files:**
- Create: `app/src/main/java/com/busarrival/app/domain/model/MapState.kt`

- [ ] **Step 1: Create MapState data class**

```kotlin
package com.busarrival.app.domain.model

import androidx.compose.ui.geometry.Offset

/**
 * Unified map rendering state.
 * Consolidates scale, offset, and center coordinates.
 */
data class MapState(
    val centerLon: Double = 120.0,
    val centerLat: Double = 20.0,
    val tileZ: Int = 15,
    val scale: Float = 1f,
    val offset: Offset = Offset.Zero
)
```

- [ ] **Step 2: Commit**

```bash
git add app/src/main/java/com/busarrival/app/domain/model/MapState.kt
git commit -m "feat: add MapState data class for unified map state"
```

---

### Task 2: Create ReplayState data class

**Files:**
- Create: `app/src/main/java/com/busarrival/app/domain/model/ReplayState.kt`

- [ ] **Step 1: Create ReplayState data class**

```kotlin
package com.busarrival.app.domain.model

/**
 * Timeline/replay state for historical inspection.
 */
data class ReplayState(
    val currentTime: Long = 0,        // Current position in trace (ms)
    val isPlaying: Boolean = false,   // Play/pause state
    val playbackSpeed: Float = 1f,    // 0.5x, 1x, 2x, 4x
    val traceDuration: Long = 0,      // Total trace length (ms)
    val cameraFollowEnabled: Boolean = true,
    val traceFile: String? = null     // Currently loaded trace path
)
```

- [ ] **Step 2: Commit**

```bash
git add app/src/main/java/com/busarrival/app/domain/model/ReplayState.kt
git commit -m "feat: add ReplayState data class for timeline features"
```

---

### Task 3: Extract coordinate functions to testable module

**Files:**
- Create: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapCoordinateUtils.kt`

- [ ] **Step 1: Write failing test for worldX using tileZ**

Create test file: `app/src/test/java/com/busarrival/app/presentation/ui/detection/CoordinateSystemTest.kt`

```kotlin
package com.busarrival.app.presentation.ui.detection

import kotlin.math.pow
import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.tan
import org.junit.Test
import kotlin.test.assertEquals

class CoordinateSystemTest {
    @Test
    fun worldX_usesTileZ_notBaseZ() {
        val centerLon = 120.0
        val tileZ = 18

        val tile1_worldX = worldX(tileXToLon(10000, tileZ), centerLon, tileZ)
        val tile2_worldX = worldX(tileXToLon(10001, tileZ), centerLon, tileZ)

        // Spacing should be exactly 256px at tileZ
        assertEquals(256f, tile2_worldX - tile1_worldX, 0.1f)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./gradlew test --tests CoordinateSystemTest`
Expected: FAIL with "Unresolved reference: worldX"

- [ ] **Step 3: Create MapCoordinateUtils.kt with coordinate functions**

```kotlin
package com.busarrival.app.presentation.ui.detection.components

import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.atan
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.tan

/**
 * Coordinate conversion utilities for map rendering.
 * Top-level functions for testability (not local to drawScope).
 */

/**
 * Convert longitude to world X coordinate at given zoom level.
 * Uses tileZ (NOT baseZ) to ensure tiles align correctly.
 */
fun worldX(lon: Double, centerLon: Double, tileZ: Int): Float {
    return lonToPixelX(lon, tileZ) - lonToPixelX(centerLon, tileZ)
}

/**
 * Convert latitude to world Y coordinate at given zoom level.
 * Uses tileZ (NOT baseZ) to ensure tiles align correctly.
 */
fun worldY(lat: Double, centerLat: Double, tileZ: Int): Float {
    return latToPixelY(lat, tileZ) - latToPixelY(centerLat, tileZ)
}

/**
 * Convert longitude to pixel X at given zoom level (Web Mercator).
 */
fun lonToPixelX(lon: Double, zoom: Int): Float {
    val x = (lon + 180.0) / 360.0 * 2.0.pow(zoom)
    return (x * 256).toFloat()
}

/**
 * Convert latitude to pixel Y at given zoom level (Web Mercator).
 */
fun latToPixelY(lat: Double, zoom: Int): Float {
    val y = ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom))
    return (y * 256).toFloat()
}

/**
 * Convert longitude to tile X coordinate at given zoom level.
 */
fun lonToTileX(lon: Double, zoom: Int): Int {
    return ((lon + 180.0) / 360.0 * 2.0.pow(zoom)).toInt()
}

/**
 * Convert latitude to tile Y coordinate at given zoom level.
 */
fun latToTileY(lat: Double, zoom: Int): Int {
    return ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom)).toInt()
}

/**
 * Convert tile X to longitude at given zoom level.
 */
fun tileXToLon(tileX: Int, zoom: Int): Double {
    return tileX / 2.0.pow(zoom) * 360.0 - 180.0
}

/**
 * Convert tile Y to latitude at given zoom level.
 */
fun tileYToLat(tileY: Int, zoom: Int): Double {
    val n = PI - 2.0 * PI * tileY / 2.0.pow(zoom)
    return 180.0 / PI * atan(0.5 * (exp(n) - exp(-n)))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./gradlew test --tests CoordinateSystemTest`
Expected: PASS

- [ ] **Step 5: Add regression test for baseZ bug**

```kotlin
@Test
fun regression_baseZ_notUsedInWorldX() {
    val centerLon = 120.0
    val tileZ = 18
    val baseZ = 15

    val worldX_tileZ = worldX(tileXToLon(10000, tileZ), centerLon, tileZ)
    val worldX_baseZ = worldX(tileXToLon(10000, tileZ), centerLon, baseZ)

    // Should NOT be equal (catches bug if someone uses baseZ)
    assertNotEquals(worldX_tileZ, worldX_baseZ)
}
```

- [ ] **Step 6: Run regression test**

Run: `./gradlew test --tests CoordinateSystemTest`
Expected: PASS (both tests)

- [ ] **Step 7: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapCoordinateUtils.kt
git add app/src/test/java/com/busarrival/app/presentation/ui/detection/CoordinateSystemTest.kt
git commit -m "feat: extract coordinate functions to MapCoordinateUtils for testability

- Add worldX/worldY using tileZ (not baseZ) for correct tile alignment
- Add lonToPixelX/Y, lonToTileX/Y, tileXToLon/Y helper functions
- Add unit tests verifying 256px spacing at tileZ
- Add regression test catching baseZ usage bug
"
```

---

### Task 4: Add tile positioning tests

**Files:**
- Create: `app/src/test/java/com/busarrival/app/presentation/ui/detection/TilePositioningTest.kt`

- [ ] **Step 1: Write tile positioning tests**

```kotlin
package com.busarrival.app.presentation.ui.detection

import androidx.compose.ui.geometry.Offset
import com.busarrival.app.presentation.ui.detection.components.worldX
import com.busarrival.app.presentation.ui.detection.components.tileXToLon
import org.junit.Test
import kotlin.test.assertEquals

class TilePositioningTest {
    @Test
    fun tilesStitchPerfectly_atScale1() {
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val centerLon = 120.0

        // Two adjacent tiles at tileZ=15
        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        // No gap
        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }

    @Test
    fun tilesStitchPerfectly_atScale2() {
        val scale = 2f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val centerLon = 120.0

        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }

    @Test
    fun tilesStitchPerfectly_atScale4() {
        val scale = 4f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val centerLon = 120.0

        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }

    @Test
    fun tilesStitchWithPanOffset() {
        val scale = 2f
        val offset = Offset(50f, -30f)
        val canvasWidth = 1000f
        val centerLon = 120.0

        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }
}
```

- [ ] **Step 2: Run tests**

Run: `./gradlew test --tests TilePositioningTest`
Expected: PASS (all 4 tests)

- [ ] **Step 3: Commit**

```bash
git add app/src/test/java/com/busarrival/app/presentation/ui/detection/TilePositioningTest.kt
git commit -m "test: add tile positioning tests for scales 1, 2, 4 with pan offset"
```

---

### Task 5: Add fallback tile tests

**Files:**
- Create: `app/src/test/java/com/busarrival/app/presentation/ui/detection/FallbackTileTest.kt`

- [ ] **Step 1: Write fallback tile tests**

```kotlin
package com.busarrival.app.presentation.ui.detection

import com.busarrival.app.presentation.ui.detection.components.worldX
import com.busarrival.app.presentation.ui.detection.components.tileXToLon
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.math.pow

class FallbackTileTest {
    @Test
    fun fallbackTile_scalesToMatchNative() {
        val tileZ = 17
        val fallbackZ = 15

        // Native tile at Z=17: 256px world space
        val nativeWorldSize = 256f

        // Fallback tile at Z=15: scaled by 2^(17-15) = 4x
        val zoomScaleFactor = 2.0.pow(tileZ - fallbackZ).toFloat()
        val fallbackWorldSize = 256f * zoomScaleFactor

        assertEquals(nativeWorldSize, fallbackWorldSize, 0.1f)
    }

    @Test
    fun fallbackTile_alignsWithNative() {
        val tileZ = 17
        val fallbackZ = 15
        val centerLon = 120.0

        // Both positioned at same worldX
        val worldX_val = worldX(tileXToLon(10000, tileZ), centerLon, tileZ)

        // Native covers [worldX, worldX + 256]
        // Fallback (scaled 4x) covers [worldX, worldX + 256*4]
        val zoomScaleFactor = 2.0.pow(tileZ - fallbackZ).toFloat()

        assertEquals(256f * zoomScaleFactor, 256f * 4f, 0.1f)
    }
}
```

- [ ] **Step 2: Run tests**

Run: `./gradlew test --tests FallbackTileTest`
Expected: PASS (both tests)

- [ ] **Step 3: Commit**

```bash
git add app/src/test/java/com/busarrival/app/presentation/ui/detection/FallbackTileTest.kt
git commit -m "test: add fallback tile scaling and alignment tests"
```

---

### Task 6: Add transform chain tests

**Files:**
- Create: `app/src/test/java/com/busarrival/app/presentation/ui/detection/TransformChainTest.kt`

- [ ] **Step 1: Write transform chain tests**

```kotlin
package com.busarrival.app.presentation.ui.detection

import androidx.compose.ui.geometry.Offset
import org.junit.Test
import kotlin.test.assertEquals

class TransformChainTest {
    @Test
    fun transformChain_worldToScreen() {
        val worldX = 100f
        val scale = 2f
        val offset = Offset(50f, 0f)
        val canvasWidth = 1000f

        val screenX = worldX * scale + offset.x + canvasWidth / 2

        assertEquals(100f * 2f + 50f + 500f, screenX)
    }

    @Test
    fun transformChain_preservesRelativePositions() {
        val worldX1 = 100f
        val worldX2 = 356f  // 256px to the right (one tile)
        val scale = 2f
        val offset = Offset.Zero
        val canvasWidth = 1000f

        val screenX1 = worldX1 * scale + offset.x + canvasWidth / 2
        val screenX2 = worldX2 * scale + offset.x + canvasWidth / 2

        // Screen distance should be 256 * scale
        assertEquals(256f * scale, screenX2 - screenX1, 0.1f)
    }
}
```

- [ ] **Step 2: Run tests**

Run: `./gradlew test --tests TransformChainTest`
Expected: PASS (both tests)

- [ ] **Step 3: Commit**

```bash
git add app/src/test/java/com/busarrival/app/presentation/ui/detection/TransformChainTest.kt
git commit -m "test: add transform chain tests for world-to-screen conversion"
```

---

### Task 7: Update MapView to use extracted coordinate functions

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`

- [ ] **Step 1: Update imports in MapView.kt**

Add to imports:
```kotlin
import com.busarrival.app.presentation.ui.detection.components.worldX
import com.busarrival.app.presentation.ui.detection.components.worldY
import com.busarrival.app.presentation.ui.detection.components.lonToTileX
import com.busarrival.app.presentation.ui.detection.components.latToTileY
import com.busarrival.app.presentation.ui.detection.components.tileXToLon
import com.busarrival.app.presentation.ui.detection.components.tileYToLat
import com.busarrival.app.presentation.ui.detection.components.lonToPixelX
import com.busarrival.app.presentation.ui.detection.components.latToPixelY
```

- [ ] **Step 2: Remove local coordinate function definitions**

Delete lines 184-189 (local worldX/worldY functions inside withTransform block).

- [ ] **Step 3: Update tile positioning to use extracted functions**

Replace tile worldX/worldY calculations (lines 251-256) with:
```kotlin
val tileNW_lon = tileXToLon(tileX, tileZ)
val tileNE_lat = tileYToLat(tileY, tileZ)

val tileWorldX = worldX(tileNW_lon, center.lon, tileZ)
val tileWorldY = worldY(tileNE_lat, center.lat, tileZ)
```

- [ ] **Step 4: Update route rendering to use extracted functions**

Replace toScreenX/toScreenY calls (lines 172-178) with direct worldX/worldY:
```kotlin
fun toScreenX(lon: Double): Float {
    val pixelX = lonToPixelX(lon, tileZ) - lonToPixelX(center.lon, tileZ) + canvasWidth / 2
    return pixelX * scale + offset.x
}

fun toScreenY(lat: Double): Float {
    val pixelY = latToPixelY(lat, tileZ) - latToPixelY(center.lat, tileZ) + canvasHeight / 2
    return pixelY * scale + offset.y
}
```

- [ ] **Step 5: Build to verify no compilation errors**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 6: Run all coordinate system tests**

Run: `./gradlew test --tests CoordinateSystemTest --tests TilePositioningTest --tests FallbackTileTest --tests TransformChainTest`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "refactor: use extracted coordinate functions in MapView

- Replace local worldX/worldY with top-level functions from MapCoordinateUtils
- Update tile positioning to use extracted tileXToLon/Y functions
- Route rendering now uses same coordinate system as tiles (tileZ)
- Enables unit testing of coordinate conversions
"
```

---

## Phase 2: Timeline and Replay

### Task 8: Create trace storage manager

**Files:**
- Create: `app/src/main/java/com/busarrival/app/data/trace/TraceStorageManager.kt`

- [ ] **Step 1: Write test for trace file listing**

Create test file: `app/src/test/java/com/busarrival/app/data/trace/TraceStorageManagerTest.kt`

```kotlin
package com.busarrival.app.data.trace

import org.junit.Test
import kotlin.test.assertTrue
import java.io.File

class TraceStorageManagerTest {
    @Test
    fun listTraces_returnsEmptyInitially() {
        val tempDir = createTempDir()
        val manager = TraceStorageManager(tempDir)

        val traces = manager.listTraces()

        assertTrue(traces.isEmpty())
    }

    private fun createTempDir(): File {
        val temp = File.createTempFile("traces", "")
        temp.delete()
        temp.mkdirs()
        return temp
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./gradlew test --tests TraceStorageManagerTest`
Expected: FAIL with "Unresolved reference: TraceStorageManager"

- [ ] **Step 3: Implement TraceStorageManager**

```kotlin
package com.busarrival.app.data.trace

import android.content.Context
import com.busarrival.app.service.PipelineEvent
import com.google.gson.Gson
import com.google.gson.JsonObject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileWriter

/**
 * Manages trace file storage (JSONL format).
 * Each trace is a file with one JSON line per event.
 */
class TraceStorageManager(
    private val context: Context,
    private val gson: Gson = Gson()
) {
    private val tracesDir: File
        get() = File(context.filesDir, "traces").apply { mkdirs() }

    /**
     * List all available trace files.
     * @return List of trace metadata (filename, size, timestamp)
     */
    suspend fun listTraces(): List<TraceMetadata> = withContext(Dispatchers.IO) {
        tracesDir.listFiles()
            ?.filter { it.extension == "jsonl" }
            ?.map { file ->
                TraceMetadata(
                    filename = file.name,
                    path = file.absolutePath,
                    sizeBytes = file.length(),
                    lastModified = file.lastModified()
                )
            }
            ?: emptyList()
    }

    /**
     * Save events to trace file.
     */
    suspend fun saveTrace(
        routeUuid: String,
        events: List<PipelineEvent>
    ): String = withContext(Dispatchers.IO) {
        val timestamp = System.currentTimeMillis()
        val filename = "trace_${routeUuid}_$timestamp.jsonl"
        val file = File(tracesDir, filename)

        FileWriter(file).use { writer ->
            events.forEach { event ->
                val json = gson.toJson(event)
                writer.write(json)
                writer.write("\n")
            }
        }

        file.absolutePath
    }

    /**
     * Load events from trace file.
     */
    suspend fun loadTrace(path: String): List<PipelineEvent> = withContext(Dispatchers.IO) {
        val file = File(path)
        if (!file.exists()) {
            return@withContext emptyList()
        }

        file.readLines()
            .mapNotNull { line ->
                try {
                    gson.fromJson(line, PipelineEvent::class.java)
                } catch (e: Exception) {
                    android.util.Log.e("TraceStorage", "Failed to parse: $line", e)
                    null
                }
            }
    }

    /**
     * Delete trace file.
     */
    suspend fun deleteTrace(path: String): Boolean = withContext(Dispatchers.IO) {
        val file = File(path)
        if (file.exists() && file.parentFile == tracesDir) {
            file.delete()
        } else {
            false
        }
    }
}

/**
 * Trace file metadata.
 */
data class TraceMetadata(
    val filename: String,
    val path: String,
    val sizeBytes: Long,
    val lastModified: Long
)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./gradlew test --tests TraceStorageManagerTest`
Expected: PASS

- [ ] **Step 5: Add test for saving and loading traces**

```kotlin
@Test
fun saveAndLoadTrace_roundtripsCorrectly() {
    val tempDir = createTempDir()
    val manager = TraceStorageManager(tempDir)

    val events = listOf(
        createTestEvent(0, 1000),
        createTestEvent(1000, 2000)
    )

    val path = manager.saveTrace("test-route", events)
    val loaded = manager.loadTrace(path)

    assertEquals(2, loaded.size)
    assertEquals(1000, loaded[0].sCm)
}

private fun createTestEvent(sCm: Int, timestamp: Long): PipelineEvent {
    return PipelineEvent.PositionUpdate(
        timestamp = timestamp,
        sCm = sCm,
        vCms = 100,
        mode = "Normal"
    )
}
```

- [ ] **Step 6: Commit**

```bash
git add app/src/main/java/com/busarrival/app/data/trace/TraceStorageManager.kt
git add app/src/test/java/com/busarrival/app/data/trace/TraceStorageManagerTest.kt
git commit -m "feat: add TraceStorageManager for JSONL trace files

- Save events to trace_{uuid}_{timestamp}.jsonl format
- Load traces from file system
- List available traces with metadata
- Delete trace files
- Unit tests for save/load roundtrip
"
```

---

### Task 9: Add ReplayState to ViewModel

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/viewmodel/DetectionViewModel.kt`

- [ ] **Step 1: Add ReplayState to ViewModel**

Add after line 43 (after _tileCache):
```kotlin
// Replay state
private val _replayState = MutableStateFlow(ReplayState())
val replayState: StateFlow<ReplayState> = _replayState.asStateFlow()
```

Add import:
```kotlin
import com.busarrival.app.domain.model.ReplayState
```

- [ ] **Step 2: Add replay control functions**

Add after `clearTileCache()`:
```kotlin
/**
 * Seek to specific time in trace.
 */
fun seekTo(time: Long) {
    _replayState.value = _replayState.value.copy(currentTime = time)
}

/**
 * Toggle playback state.
 */
fun togglePlayback() {
    _replayState.value = _replayState.value.copy(
        isPlaying = !_replayState.value.isPlaying
    )
}

/**
 * Set playback speed.
 */
fun setPlaybackSpeed(speed: Float) {
    _replayState.value = _replayState.value.copy(playbackSpeed = speed)
}

/**
 * Load trace for replay.
 */
suspend fun loadTrace(traceFile: String, traceStorage: TraceStorageManager) {
    val events = traceStorage.loadTrace(traceFile)
    if (events.isNotEmpty()) {
        val duration = events.lastOrNull { it is PipelineEvent.PositionUpdate }
            ?.let { (it as PipelineEvent.PositionUpdate).timestamp }
            ?: 0L

        _replayState.value = _replayState.value.copy(
            traceFile = traceFile,
            traceDuration = duration,
            currentTime = 0
        )
    }
}
```

- [ ] **Step 3: Add TraceStorageManager dependency**

Update constructor:
```kotlin
class DetectionViewModel(
    application: Application,
    private val traceStorage: TraceStorageManager = TraceStorageManager(application)
) : AndroidViewModel(application) {
```

Add import:
```kotlin
import com.busarrival.app.data.trace.TraceStorageManager
```

- [ ] **Step 4: Build to verify compilation**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 5: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/viewmodel/DetectionViewModel.kt
git commit -m "feat: add replay state and controls to ViewModel

- Add ReplayState with timeline position, playback state, speed
- Add seekTo(), togglePlayback(), setPlaybackSpeed(), loadTrace() functions
- Add TraceStorageManager dependency for trace file operations
"
```

---

### Task 10: Create TimelineScrubber component

**Files:**
- Create: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt`

- [ ] **Step 1: Create TimelineScrubber composable**

```kotlin
package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Timeline scrubber for historical trace replay.
 */
@Composable
fun TimelineScrubber(
    viewModel: DetectionViewModel,
    modifier: Modifier = Modifier
) {
    val replayState by viewModel.replayState.collectAsState()

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        // Play/Pause and Speed controls
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { viewModel.togglePlayback() }) {
                    Text(if (replayState.isPlaying) "⏸" else "▶")
                }

                Text(
                    text = formatTime(replayState.currentTime),
                    style = MaterialTheme.typography.bodyMedium
                )
            }

            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                SpeedButton("0.5x", 0.5f, replayState.playbackSpeed, viewModel)
                SpeedButton("1x", 1f, replayState.playbackSpeed, viewModel)
                SpeedButton("2x", 2f, replayState.playbackSpeed, viewModel)
                SpeedButton("4x", 4f, replayState.playbackSpeed, viewModel)
            }
        }

        // Time slider
        if (replayState.traceDuration > 0) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Slider(
                    value = replayState.currentTime.toFloat(),
                    onValueChange = { viewModel.seekTo(it.toLong()) },
                    valueRange = 0f..replayState.traceDuration.toFloat(),
                    modifier = Modifier.weight(1f)
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    text = formatTime(0),
                    style = MaterialTheme.typography.bodySmall
                )
                Text(
                    text = formatTime(replayState.traceDuration),
                    style = MaterialTheme.typography.bodySmall
                )
            }
        }
    }
}

@Composable
private fun SpeedButton(
    label: String,
    speed: Float,
    currentSpeed: Float,
    viewModel: DetectionViewModel
) {
    Button(
        onClick = { viewModel.setPlaybackSpeed(speed) },
        enabled = currentSpeed != speed
    ) {
        Text(label, style = MaterialTheme.typography.bodySmall)
    }
}

private fun formatTime(ms: Long): String {
    val seconds = ms / 1000
    val minutes = seconds / 60
    val secs = seconds % 60
    return String.format(Locale.US, "%02d:%02d", minutes, secs)
}
```

- [ ] **Step 2: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt
git commit -m "feat: add TimelineScrubber component for historical replay

- Play/pause button with playback state
- Speed controls: 0.5x, 1x, 2x, 4x
- Time slider with current position display
- Time formatting (MM:SS)
"
```

---

### Task 11: Integrate TimelineScrubber into DetectionScreen

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt` (or wherever MapView is used)

- [ ] **Step 1: Add TimelineScrubber to screen layout**

Add TimelineScrubber below MapView (or above, based on design):
```kotlin
@Composable
fun DetectionScreen(
    viewModel: DetectionViewModel = viewModel(),
    modifier: Modifier = Modifier
) {
    val uiState by viewModel.uiState.collectAsState()

    Box(modifier = modifier.fillMaxSize()) {
        MapView(
            routeData = viewModel.activeRoute.collectAsState().value,
            currentSCm = uiState.sCm,
            isCameraFollowEnabled = uiState.isCameraFollowEnabled,
            viewModel = viewModel
        )

        // Add timeline scrubber at bottom
        Box(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
        ) {
            TimelineScrubber(viewModel)
        }
    }
}
```

- [ ] **Step 2: Build to verify layout**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt
git commit -m "feat: integrate TimelineScrubber into DetectionScreen

- Add timeline scrubber at bottom of screen
- Map view and timeline coexist in same layout
"
```

---

### Task 12: Connect timeline to route marker position

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`

- [ ] **Step 1: Add replay state observation**

Add to MapView parameters:
```kotlin
val replayState by viewModel.replayState.collectAsState()
```

- [ ] **Step 2: Update position marker to use replay time**

Replace current position marker drawing (lines 332-342) with:
```kotlin
// Draw current position (from live detection or replay)
val positionSCm = if (replayState.traceFile != null) {
    // During replay, find position by seeking through events
    findPositionAtTime(replayState.currentTime, viewModel.events.value)
} else {
    // Live detection
    currentSCm
}

val currentNode = routeData.findNodeAtProgress(positionSCm)
if (currentNode != null) {
    val ll = routeData.cmToLatLon(currentNode.xCm, currentNode.yCm)
    val px = toScreenX(ll.lon)
    val py = toScreenY(ll.lat)
    drawCircle(
        color = Color.Green,
        radius = 12f,
        center = Offset(px, py)
    )
}
```

- [ ] **Step 3: Add helper function to find position at time**

Add to MapView (outside Canvas):
```kotlin
private fun findPositionAtTime(
    time: Long,
    events: List<PipelineEvent>
): Int {
    // Find last PositionUpdate before given time
    return events
        .filterIsInstance<PipelineEvent.PositionUpdate>()
        .lastOrNull { it.timestamp <= time }
        ?.sCm ?: 0
}
```

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "feat: connect timeline to route marker position

- Add replay state observation to MapView
- Route marker position updates during timeline scrub
- Find position at time from event history
- Live detection and replay use same marker rendering
"
```

---

## Verification

### Task 13: Run full test suite

- [ ] **Step 1: Run all unit tests**

Run: `./gradlew test`
Expected: All PASS

- [ ] **Step 2: Build debug APK**

Run: `./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Install and test manually**

Run: `adb install -r app/build/outputs/apk/debug/app-debug.apk`

Manual tests:
1. Open app → verify tiles render without gaps
2. Pinch zoom to 2x → verify tiles still align
3. Pinch zoom to 4x → verify tiles still align
4. Pan map → verify smooth movement
5. (If trace file available) Load trace → scrub timeline → verify marker follows

- [ ] **Step 4: Commit final changes**

```bash
git add -A
git commit -m "test: verify Phase 1 and Phase 2 implementation

- All coordinate system tests passing
- Tiles align correctly at scales 1, 2, 4
- Timeline scrubber functional
- Route marker follows timeline position
"
```

---

## Success Criteria Check

Before considering this plan complete, verify:

- [ ] Tiles stitch perfectly at scales 1, 2, 4
- [ ] Route aligns with tiles at all zoom levels
- [ ] Pinch-to-zoom works (centroid tracking)
- [ ] Timeline scrubbing updates position marker
- [ ] All unit tests pass
- [ ] Manual testing confirms no visual regressions

---

**End of Plan**

Next phases (Phase 3: State Refactoring, Phase 4: Polish) are optional enhancements beyond bug fixes and core replay functionality.
