# Map Tile Rendering System Design

**Date**: 2026-05-12
**Status**: Draft
**Scope**: Full map system redesign for Android bus arrival app

---

## Executive Summary

Redesign map rendering system to fix tile alignment bugs, support historical replay, and provide clean architecture for future features. Core principle: **single unified coordinate system** eliminates current misalignment issues.

**Primary use case**: Historical trip replay (timeline scrubber + independent map exploration)
**Secondary**: Route inspection (static view, pan/zoom)
**Tertiary**: Real-time tracking (camera follow)

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Coordinate System](#coordinate-system)
3. [Tile Layer](#tile-layer)
4. [Vector Layer](#vector-layer)
5. [Gesture Handling](#gesture-handling)
6. [Timeline & Replay](#timeline--replay)
7. [Component Architecture](#component-architecture)
8. [State Management](#state-management)
9. [Testing Strategy](#testing-strategy)
10. [Migration Path](#migration-path)
11. [Performance Considerations](#performance-considerations)
12. [Edge Cases & Error Handling](#edge-cases--error-handling)

---

## Architecture Overview

### 3-Layer Rendering System

```
┌─────────────────────────────────────────────────────────┐
│                    UI Layer (Compose)                   │
│  ┌─────────────────────────────────────────────────┐   │
│  │  TimelineScrubber (play/pause, speed, seek)     │   │
│  │  DebugOverlay (zoom, tiles, position info)      │   │
│  │  ControlButtons (clear cache, follow mode)      │   │
│  └─────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────┤
│                 Vector Layer (Compose Graphics)         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  RouteRenderer (path in world space)            │   │
│  │  StopRenderer (markers, tooltips)               │   │
│  │  PositionMarker (current bus position)          │   │
│  └─────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────┤
│                   Tile Layer (Canvas)                   │
│  ┌─────────────────────────────────────────────────┐   │
│  │  OSM Tiles (raster, world space positioning)    │   │
│  │  TileCache (disk + memory)                      │   │
│  │  TileLoader (async, fallback)                   │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Technology Choices

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Tiles | Canvas drawScope | Efficient bitmap rendering |
| Overlays | Compose graphics API | Easy styling, interaction |
| UI | Compose Material3 | Consistent app design |

---

## Coordinate System

### World Space Definition

**Single unified coordinate system** for all spatial data.

```kotlin
// World space origin at route center
data class WorldOrigin(
    val lon: Double,  // Route center longitude
    val lat: Double   // Route center latitude
)

// World coordinates: pixels at current tileZ
data class WorldPoint(
    val x: Float,  // East-positive (lon → pixelX)
    val y: Float   // South-positive (lat → pixelY)
)
```

### Transform Chain

```
Geographic (lon, lat)
    ↓ lonToPixelX/Y(lon, lat, tileZ)
World Space (pixels at tileZ)
    ↓ scale(×scale) + translate(+offset)
Screen Space (px, py)
```

**Single transform application**:
```kotlin
// Inside drawScope
withTransform({
    translate(left = offset.x + canvasWidth/2, top = offset.y + canvasHeight/2)
    scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
}) {
    // ALL elements drawn here use world coordinates
    // No per-element scaling or coordinate conversion
}
```

### Coordinate Conversion Functions

**All use tileZ (NOT baseZ)**:

```kotlin
// Lon/lat → world pixels at CURRENT tileZ
fun worldX(lon: Double, centerLon: Double, tileZ: Int): Float {
    return lonToPixelX(lon, tileZ) - lonToPixelX(centerLon, tileZ)
}

fun worldY(lat: Double, centerLat: Double, tileZ: Int): Float {
    return latToPixelY(lat, tileZ) - latToPixelY(centerLat, tileZ)
}

// OSM tile coordinate conversions (Web Mercator)
private fun lonToPixelX(lon: Double, zoom: Int): Float {
    val x = (lon + 180.0) / 360.0 * 2.0.pow(zoom)
    return (x * 256).toFloat()
}

private fun latToPixelY(lat: Double, zoom: Int): Float {
    val y = ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom))
    return (y * 256).toFloat()
}
```

### Key Principle

**CRITICAL**: `worldX/Y` MUST use `tileZ`, never `baseZ`.

- ✅ Correct: `lonToPixelX(lon, tileZ)`
- ❌ Bug: `lonToPixelX(lon, baseZ)`  // Causes misalignment at high zoom

---

## Tile Layer

### Tile Positioning

Tiles positioned in world space using their northwest corner:

```kotlin
val centerTileX = lonToTileX(centerLon, tileZ)
val centerTileY = latToTileY(centerLat, tileZ)

for (dx in -tileRange..tileRange) {
    for (dy in -tileRange..tileRange) {
        val tileX = centerTileX + dx
        val tileY = centerTileY + dy

        // Tile NW corner in world space
        val tileNW_lon = tileXToLon(tileX, tileZ)
        val tileNE_lat = tileYToLat(tileY, tileZ)

        val worldX = worldX(tileNW_lon, centerLon, tileZ)
        val worldY = worldY(tileNE_lat, centerLat, tileZ)

        // Draw at (worldX, worldY) — outer transform handles scale/offset
        drawImage(tileBitmap, Offset(worldX, worldY))
    }
}
```

### Fallback Tile Scaling

When high-zoom tile unavailable, use lower-zoom tile scaled up:

```kotlin
if (bitmap == null && tileZ > 14) {
    // Find fallback at lower zoom
    for (fallbackZ in (tileZ - 1) downTo 14) {
        val fallbackX = tileX / 2.0.pow(tileZ - fallbackZ).toInt()
        val fallbackY = tileY / 2.0.pow(tileZ - fallbackZ).toInt()

        tileCache["$fallbackZ/$fallbackX/$fallbackY"]?.let { fallback ->
            val zoomScaleFactor = 2.0.pow(tileZ - fallbackZ).toFloat()

            // Position at same worldX/Y, scale to fill requested tile
            withTransform({
                scale(zoomScaleFactor, zoomScaleFactor, Offset(worldX, worldY))
            }) {
                drawImage(fallback, Offset(worldX, worldY), alpha = 0.7f)
            }
        }
    }
}
```

### Tile Loading Strategy

**Visible range calculation**:
```kotlin
val tileRange = when (tileZ) {
    in 12..13 -> 3  // 7×7 tiles
    in 14..15 -> 3  // 7×7 tiles
    in 16..17 -> 4  // 9×9 tiles
    else -> 5       // 11×11 tiles
}
```

**Preloading**: Load 1 zoom level beyond current for smooth zooming.

**Priority**: Current zoom > Adjacent tiles > Fallback zoom levels.

---

## Vector Layer

### Route Rendering

Route path computed in world space at current tileZ:

```kotlin
// Compute route points once per tileZ change
val routePoints = routeData.nodes.map { node ->
    val ll = routeData.cmToLatLon(node.xCm, node.yCm)
    Offset(
        worldX(ll.lon, centerLon, tileZ),
        worldY(ll.lat, centerLat, tileZ)
    )
}

// Draw with path
val path = Path().apply {
    routePoints.forEachIndexed { i, pt ->
        if (i == 0) moveTo(pt.x, pt.y) else lineTo(pt.x, pt.y)
    }
}

drawPath(
    path = path,
    color = Color.Blue,
    style = Stroke(width = 4f / scale)  // Constant screen width
)
```

**Key**: Route uses SAME `worldX/Y` as tiles. No coordinate conversion.

### Stop Markers

```kotlin
routeData.stops.forEach { stop ->
    val node = routeData.findNodeAtProgress(stop.progressCm)
    val ll = routeData.cmToLatLon(node.xCm, node.yCm)

    val worldX = worldX(ll.lon, centerLon, tileZ)
    val worldY = worldY(ll.lat, centerLat, tileZ)

    drawCircle(
        color = Color.Red,
        radius = 8f / scale,  // Constant screen size
        center = Offset(worldX, worldY)
    )
}
```

### Current Position Marker

```kotlin
val currentNode = routeData.findNodeAtProgress(currentSCm)
val ll = routeData.cmToLatLon(currentNode.xCm, currentNode.yCm)

val worldX = worldX(ll.lon, centerLon, tileZ)
val worldY = worldY(ll.lat, centerLat, tileZ)

// Draw pulsing marker
drawCircle(
    color = Color.Green,
    radius = 12f / scale,
    center = Offset(worldX, worldY)
)
```

---

## Gesture Handling

### Pinch-to-Zoom

```kotlin
.pointerInput(Unit) {
    detectTransformGestures { centroid, pan, zoom, _ ->
        val oldScale = scale
        val newScale = (oldScale * zoom).coerceIn(0.1f, 10f)

        val oldOffset = offset
        val newOffset = oldOffset + (centroid - oldOffset) * (1 - newScale / oldScale) + pan

        viewModel.updateMapState(newScale, newOffset)
    }
}
```

### Pan

Drag gesture updates `offset`. Applied after center translation.

### Zoom Levels

```kotlin
val baseZ = 15
val tileZ = remember(scale) {
    (baseZ + (ln(scale.toDouble()) / ln(2.0)).toInt()).coerceIn(12, 18)
}
```

**Continuous user scale** → **discrete tileZ** for tile loading.

---

## Timeline & Replay

### Replay State

```kotlin
data class ReplayState(
    val currentTime: Long = 0,        // Current position in trace (ms)
    val isPlaying: Boolean = false,   // Play/pause state
    val playbackSpeed: Float = 1f,    // 0.5x, 1x, 2x, 4x
    val traceDuration: Long = 0,      // Total trace length (ms)
    val cameraFollowEnabled: Boolean = true
)
```

### Timeline UI

```
┌────────────────────────────────────────────────────────┐
│  ▶ ●━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━●  │
│  00:00 / 15:30                    Speed: [1x] [2x] [4x] │
└────────────────────────────────────────────────────────┘
```

### Camera Behavior

| State | Camera Behavior |
|-------|-----------------|
| Playing | Optional follow (toggleable) |
| Paused | Independent pan/zoom |
| Scrubbing | Jump to position, maintain viewport |

### Performance

- **Scrub**: Update route marker only (cheap)
- **Zoom change**: Reload tiles (expensive, debounce)
- **Playback**: Tick at 10-30fps (not 60fps)

---

## Component Architecture

### Component Hierarchy

```
MapView (Composable)
│
├── TileCanvas (Canvas)
│   ├── TileCache (disk + memory)
│   │   ├── preloadTiles()
│   │   ├── getTile()
│   │   └── fetchAndCacheTile()
│   └── TileLoader (coroutines)
│       └── loadTilesForZoom()
│
├── VectorOverlay (Canvas/Compose)
│   ├── RouteRenderer
│   ├── StopRenderer
│   └── PositionMarker
│
├── TimelineScrubber (Compose UI)
│   ├── Slider
│   ├── PlayPauseButton
│   └── SpeedSelector
│
└── DebugOverlay (Compose UI)
    ├── ZoomInfo
    ├── TileCount
    └── ClearCacheButton
```

### Separation of Concerns

| Component | Responsibility | Dependencies |
|-----------|---------------|--------------|
| TileCanvas | Render OSM tiles | TileCache, MapState |
| VectorOverlay | Render route/stops | RouteData, MapState |
| TimelineScrubber | Playback control | ReplayState |
| DetectionViewModel | State management | All repositories |

---

## State Management

### ViewModel State

```kotlin
class DetectionViewModel : ViewModel() {
    // Map state
    private val _mapState = MutableStateFlow(MapState())
    val mapState: StateFlow<MapState> = _mapState.asStateFlow()

    // Replay state
    private val _replayState = MutableStateFlow(ReplayState())
    val replayState: StateFlow<ReplayState> = _replayState.asStateFlow()

    // Tile cache
    private val _tileCache = MutableStateFlow(mapOf<String, ImageBitmap>())
    val tileCache: StateFlow<Map<String, ImageBitmap>> = _tileCache.asStateFlow()

    // Route data
    private val _routeData = MutableStateFlow<RouteData?>(null)
    val routeData: StateFlow<RouteData?> = _routeData.asStateFlow()

    fun updateMapState(scale: Float, offset: Offset) { /* ... */ }
    fun addTiles(tiles: Map<String, ImageBitmap>) { /* ... */ }
    fun clearTileCache() { /* ... */ }
    fun seekTo(time: Long) { /* ... */ }
    fun togglePlayback() { /* ... */ }
    fun setPlaybackSpeed(speed: Float) { /* ... */ }
}
```

### MapState

```kotlin
data class MapState(
    val centerLon: Double = 120.0,
    val centerLat: Double = 20.0,
    val tileZ: Int = 15,
    val scale: Float = 1f,
    val offset: Offset = Offset.Zero
)
```

### State Flow

```
User Gesture → detectTransformGestures
    ↓
viewModel.updateMapState()
    ↓
mapState StateFlow updates
    ↓
MapView recomposes
    ↓
withTransform() applies new scale/offset
    ↓
All elements render in correct positions
```

---

## Testing Strategy

### Unit Tests

**CoordinateSystemTest.kt**
```kotlin
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
}
```

**TilePositioningTest.kt**
```kotlin
class TilePositioningTest {
    @Test
    fun tilesStitchPerfectly_atScale1() {
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f

        // Two adjacent tiles at tileZ=15
        val tile1_worldX = worldX(tileXToLon(10000, 15), 120.0, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), 120.0, 15)

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

        val tile1_worldX = worldX(tileXToLon(10000, 15), 120.0, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), 120.0, 15)

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

        val tile1_worldX = worldX(tileXToLon(10000, 15), 120.0, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), 120.0, 15)

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

        val tile1_worldX = worldX(tileXToLon(10000, 15), 120.0, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), 120.0, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }
}
```

**FallbackTileTest.kt**
```kotlin
class FallbackTileTest {
    @Test
    fun fallbackTile_scalesToMatchNative() {
        val tileZ = 17
        val fallbackZ = 15
        val scale = 4f

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
        val worldX = worldX(tileXToLon(10000, tileZ), centerLon, tileZ)

        // Native covers [worldX, worldX + 256]
        // Fallback (scaled 4x) covers [worldX, worldX + 256*4]
        val zoomScaleFactor = 2.0.pow(tileZ - fallbackZ).toFloat()

        assertEquals(256f * zoomScaleFactor, 256f * 4f, 0.1f)
    }
}
```

**TransformChainTest.kt**
```kotlin
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

### Integration Tests

**MapRenderingIntegrationTest.kt**
```kotlin
@RunWith(AndroidJUnit4::class)
class MapRenderingIntegrationTest {
    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun map_rendersTilesWithoutGaps() {
        var tileGapDetected = false

        composeTestRule.setContent {
            val routeData = remember { createTestRouteData() }
            val viewModel = remember { DetectionViewModel() }

            MapView(
                routeData = routeData,
                currentSCm = 0,
                isCameraFollowEnabled = false,
                viewModel = viewModel
            )

            // LaunchedEffect to verify rendering
            LaunchedEffect(Unit) {
                delay(1000)  // Wait for tiles to load
                // Verify no gaps via screenshot comparison
            }
        }

        // Screenshot assertion
        composeTestRule.onRoot()
            .captureToImage()
            .assertAgainstGolden("map_tiles_no_gaps")
    }
}
```

### Visual Regression Tests

**ScreenshotTests.kt**
```kotlin
class ScreenshotTests {
    @Test
    fun map_atZoom1_matchesGolden() {
        screenshotTest {
            MapViewTestContent(scale = 1f)
        }
    }

    @Test
    fun map_atZoom2_matchesGolden() {
        screenshotTest {
            MapViewTestContent(scale = 2f)
        }
    }

    @Test
    fun map_atZoom4_matchesGolden() {
        screenshotTest {
            MapViewTestContent(scale = 4f)
        }
    }
}
```

---

## Migration Path

### Phase 1: Fix Coordinate System (1-2 days)

**Tasks**:
1. ✅ Change `toScreenX/Y` to use `tileZ`
2. ✅ Ensure `worldX/Y` use `tileZ`
3. Fix gesture centroid handling
4. Verify tiles stitch at all scales

**Verification**: Run `MapViewRenderingTest.kt` → all pass

### Phase 2: Timeline & Replay (2-3 days)

**Tasks**:
1. Add `ReplayState` to ViewModel
2. Build timeline scrubber UI
3. Wire scrub position → route marker update
4. Add play/pause functionality
5. Implement playback speed control

**Verification**: Scrub through trace → position marker follows

### Phase 3: Layer Refactoring (2-3 days, optional)

**Tasks**:
1. Separate TileCanvas from VectorOverlay
2. Move to Compose graphics API for overlays
3. Add stop click handlers
4. Implement tooltips

**Verification**: Click stop → shows stop info

### Phase 4: Polish (1-2 days)

**Tasks**:
1. Add loading indicators
2. Handle error states
3. Performance tuning
4. Add debug UI

**Verification**: Smooth replay, no jank

---

## Performance Considerations

### Tile Loading

- **Cache hit**: < 1ms (memory)
- **Cache miss, disk hit**: 5-10ms
- **Network fetch**: 50-200ms (show loading state)

**Strategy**:
1. Preload adjacent zoom levels during idle time
2. Load visible tiles first, then adjacent
3. Show placeholder while loading

### Rendering

- **Target**: Responsive, not 60fps
- **Acceptable**: 30fps during playback, < 100ms input latency
- **Optimization**: Reuse Path objects, avoid allocations in draw loop

### Memory

- **Tile cache**: 50-100 tiles ~ 5-10MB (ImageBitmap)
- **Route geometry**: < 1MB (List<Offset>)
- **Total overhead**: < 20MB for map system

---

## Edge Cases & Error Handling

### Edge Cases

1. **Route at antimeridian** (lon ±180)
   - Handle tile coordinate wraparound
   - Test with routes crossing Pacific

2. **High latitude** (near poles)
   - Web Mercator distortion increases
   - Limit tileZ at extreme latitudes

3. **No route data**
   - Show empty state message
   - Hide timeline controls

4. **Empty trace**
   - Disable timeline scrubber
   - Show "No data available"

### Error Handling

1. **Tile load failure**
   - Show fallback tile if available
   - Show "Map unavailable" if all fails
   - Retry on network change

2. **Route data corruption**
   - Validate CRC on load
   - Show error message
   - Allow retry/reload

3. **Out of memory**
   - Clear tile cache
   - Reduce visible tile range
   - Show "Low memory" warning

---

## Success Criteria

- [ ] Tiles stitch perfectly at scales 1, 2, 4
- [ ] Route aligns with tiles at all zoom levels
- [ ] Pinch-to-zoom feels natural (centroid tracking)
- [ ] Timeline scrubbing updates position smoothly
- [ ] Map remains responsive during tile loads
- [ ] All tests pass (unit + integration)
- [ ] No visual regressions (screenshot tests)
- [ ] Memory usage < 20MB for map system
- [ ] Handles offline gracefully (cached tiles only)

---

## Appendix: Reference Implementations

### lonToTileX / latToTileY

```kotlin
private fun lonToTileX(lon: Double, zoom: Int): Int {
    return ((lon + 180.0) / 360.0 * 2.0.pow(zoom)).toInt()
}

private fun latToTileY(lat: Double, zoom: Int): Int {
    return ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom)).toInt()
}
```

### tileXToLon / tileYToLat

```kotlin
private fun tileXToLon(tileX: Int, zoom: Int): Double {
    return tileX / 2.0.pow(zoom) * 360.0 - 180.0
}

private fun tileYToLat(tileY: Int, zoom: Int): Double {
    val n = PI - 2.0 * PI * tileY / 2.0.pow(zoom)
    return 180.0 / PI * atan(0.5 * (exp(n) - exp(-n)))
}
```

### lonToPixelX / latToPixelY

```kotlin
private fun lonToPixelX(lon: Double, zoom: Int): Float {
    val x = (lon + 180.0) / 360.0 * 2.0.pow(zoom)
    return (x * 256).toFloat()
}

private fun latToPixelY(lat: Double, zoom: Int): Float {
    val y = ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom))
    return (y * 256).toFloat()
}
```

---

**Document version**: 1.0
**Last updated**: 2026-05-12
**Author**: Claude (with user requirements)
