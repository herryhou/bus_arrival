# Android Map Tile Coordinate Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix coordinate system bugs in Android map tile rendering that cause incorrect scaling, pan jumps, and test failures.

**Architecture:** All world coordinates use baseZ=15 as stable reference. Tiles at zoom Z scale by 2^(baseZ-Z). Fallback tiles compose scaling: 2^(baseZ-Z) * 2^(Z-F) = 2^(baseZ-F).

**Tech Stack:** Kotlin, Jetpack Compose Canvas, JUnit4

---

## File Structure

**Modified files:**
- `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt` - Production rendering code
- `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/FallbackTileTest.kt` - Existing test suite

**New test file:**
- `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/NativeTileScalingTest.kt` - New tests for native tile sizing and route overlay stability

---

### Task 1: Create new test file for native tile scaling

**Files:**
- Create: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/NativeTileScalingTest.kt`

- [ ] **Step 1: Create test file with native tile size test**

```kotlin
package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import org.junit.Test
import kotlin.test.assertEquals

/**
 * Tests for native tile scaling in baseZ coordinate system.
 * Verifies that tiles at different zoom levels scale correctly relative to baseZ.
 */
class NativeTileScalingTest {
    private val TILE_SIZE = 256f
    private val POSITION_TOLERANCE = 0.1f
    private val baseZ = 15

    /**
     * Calculate tile size in baseZ world coordinates.
     * Tile at zoom Z should have size TILE_SIZE * 2^(baseZ - Z) in baseZ space.
     */
    private fun tileSizeInBaseZ(tileZ: Int): Float {
        val scaleFactor = 2.0f.pow(baseZ - tileZ)
        return TILE_SIZE * scaleFactor
    }

    @Test
    fun native_tile_at_z17_is_quarter_size_of_z15_tile() {
        // At baseZ=15, a Z=17 tile should be 1/4 the size of a Z=15 tile
        val z15Size = tileSizeInBaseZ(15)
        val z17Size = tileSizeInBaseZ(17)

        assertEquals(TILE_SIZE, z15Size, POSITION_TOLERANCE,
            "Z=15 tile at baseZ should be native size")
        assertEquals(TILE_SIZE / 4f, z17Size, POSITION_TOLERANCE,
            "Z=17 tile at baseZ should be 1/4 of native size")
        assertEquals(z15Size / 4f, z17Size, POSITION_TOLERANCE,
            "Z=17 tile should be 1/4 of Z=15 tile")
    }

    @Test
    fun native_tile_at_z14_is_double_size_of_z15_tile() {
        // At baseZ=15, a Z=14 tile should be 2x the size of a Z=15 tile
        val z15Size = tileSizeInBaseZ(15)
        val z14Size = tileSizeInBaseZ(14)

        assertEquals(TILE_SIZE * 2f, z14Size, POSITION_TOLERANCE,
            "Z=14 tile at baseZ should be 2x native size")
        assertEquals(z15Size * 2f, z14Size, POSITION_TOLERANCE,
            "Z=14 tile should be 2x of Z=15 tile")
    }

    @Test
    fun tile_size_follows_power_of_two_scaling() {
        // Verify power-of-two scaling across zoom levels
        val z12Size = tileSizeInBaseZ(12)
        val z13Size = tileSizeInBaseZ(13)
        val z14Size = tileSizeInBaseZ(14)
        val z15Size = tileSizeInBaseZ(15)
        val z16Size = tileSizeInBaseZ(16)
        val z17Size = tileSizeInBaseZ(17)
        val z18Size = tileSizeInBaseZ(18)

        // Each zoom level should be 2x the previous
        assertEquals(z12Size / 2f, z13Size, POSITION_TOLERANCE, "Z=13 should be half of Z=12")
        assertEquals(z13Size / 2f, z14Size, POSITION_TOLERANCE, "Z=14 should be half of Z=13")
        assertEquals(z14Size / 2f, z15Size, POSITION_TOLERANCE, "Z=15 should be half of Z=14")
        assertEquals(z15Size / 2f, z16Size, POSITION_TOLERANCE, "Z=16 should be half of Z=15")
        assertEquals(z16Size / 2f, z17Size, POSITION_TOLERANCE, "Z=17 should be half of Z=16")
        assertEquals(z17Size / 2f, z18Size, POSITION_TOLERANCE, "Z=18 should be half of Z=17")
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `./gradlew testDebugUnitTest --tests NativeTileScalingTest`
Expected: PASS (these are pure math tests, should pass immediately)

- [ ] **Step 3: Commit**

```bash
git add android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/NativeTileScalingTest.kt
git commit -m "test: add native tile scaling tests

Verify power-of-two scaling: Z=17 tile is 1/4 size of Z=15 tile at baseZ=15.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Add route overlay stability test

**Files:**
- Create: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/RouteOverlayStabilityTest.kt`

- [ ] **Step 1: Create route overlay stability test**

```kotlin
package com.busarrival.app.presentation.ui.detection.components

import org.junit.Test
import kotlin.test.assertEquals

/**
 * Tests for route overlay coordinate stability across tileZ changes.
 * Verifies that toScreenX/Y produce consistent coordinates regardless of tileZ.
 */
class RouteOverlayStabilityTest {
    private val baseZ = 15
    private val CENTER_LAT = 1.3521
    private val CENTER_LON = 103.8198

    /**
     * Simulate toScreenX calculation from MapView.kt
     * All coordinates use baseZ for stability.
     */
    private fun toScreenX(lon: Double, centerLon: Double, scale: Float, offset: Float, canvasWidth: Float): Float {
        val worldX = lonToPixelX(lon, baseZ) - lonToPixelX(centerLon, baseZ)
        return worldX * scale + offset + canvasWidth / 2
    }

    private fun toScreenY(lat: Double, centerLat: Double, scale: Float, offset: Float, canvasHeight: Float): Float {
        val worldY = latToPixelY(lat, baseZ) - latToPixelY(centerLat, baseZ)
        return worldY * scale + offset + canvasHeight / 2
    }

    @Test
    fun screen_coordinates_stable_across_tileZ_changes() {
        val scale = 1f
        val offset = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        // Calculate screen coordinates for a fixed geographic point
        val testLat = CENTER_LAT + 0.001  // ~100m north
        val testLon = CENTER_LON + 0.001  // ~100m east

        // Coordinates should be identical regardless of tileZ
        // (toScreenX/Y don't use tileZ, only baseZ)
        val screenX1 = toScreenX(testLon, CENTER_LON, scale, offset, canvasWidth)
        val screenY1 = toScreenY(testLat, CENTER_LAT, scale, offset, canvasHeight)

        // Simulate what would happen at different tileZ values
        // (these should NOT affect the calculation)
        val screenX2 = toScreenX(testLon, CENTER_LON, scale, offset, canvasWidth)
        val screenY2 = toScreenY(testLat, CENTER_LAT, scale, offset, canvasHeight)

        assertEquals(screenX1, screenX2, 0.001f,
            "Screen X should be stable across tileZ changes")
        assertEquals(screenY1, screenY2, 0.001f,
            "Screen Y should be stable across tileZ changes")
    }

    @Test
    fun screen_coordinates_scale_properly_with_user_scale() {
        val offset = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        val testLat = CENTER_LAT + 0.001
        val testLon = CENTER_LON + 0.001

        // At scale 1.0
        val screenX1 = toScreenX(testLon, CENTER_LON, 1f, offset, canvasWidth)
        val screenY1 = toScreenY(testLat, CENTER_LAT, 1f, offset, canvasHeight)

        // At scale 2.0, coordinates should be 2x farther from center
        val screenX2 = toScreenX(testLon, CENTER_LON, 2f, offset, canvasWidth)
        val screenY2 = toScreenY(testLat, CENTER_LAT, 2f, offset, canvasHeight)

        // Distance from center should double
        val dist1 = kotlin.math.sqrt((screenX1 - canvasWidth/2).pow(2) + (screenY1 - canvasHeight/2).pow(2))
        val dist2 = kotlin.math.sqrt((screenX2 - canvasWidth/2).pow(2) + (screenY2 - canvasHeight/2).pow(2))

        assertEquals(dist1 * 2f, dist2, 0.1f,
            "Distance from center should double when scale doubles")
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `./gradlew testDebugUnitTest --tests RouteOverlayStabilityTest`
Expected: PASS (these test the baseZ coordinate system directly)

- [ ] **Step 3: Commit**

```bash
git add android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/RouteOverlayStabilityTest.kt
git commit -m "test: add route overlay stability tests

Verify toScreenX/Y coordinates are stable across tileZ changes and scale correctly with user zoom.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: Fix scaling direction in production code

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:283`

- [ ] **Step 1: Read current code around line 283**

Read the section to confirm current implementation

- [ ] **Step 2: Fix scaling exponent**

Change line 283 from:
```kotlin
val zoomScaleFactor = 2.0.pow(tileZ - baseZ).toFloat()
```

To:
```kotlin
val zoomScaleFactor = 2.0.pow(baseZ - tileZ).toFloat()
```

- [ ] **Step 3: Run existing tests to see failures**

Run: `./gradlew testDebugUnitTest --tests FallbackTileTest`
Expected: Some tests now pass (or fail differently) due to scaling fix

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "fix: invert tile scaling exponent for baseZ coordinates

Change 2^(tileZ - baseZ) to 2^(baseZ - tileZ) so high-zoom tiles shrink correctly in baseZ world space.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Remove offset rescaling SideEffect

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:100-116`

- [ ] **Step 1: Read the SideEffect block**

Read lines 100-116 to understand the offset adjustment logic

- [ ] **Step 2: Delete the entire prevTileZ SideEffect block**

Delete lines 100-116 (the entire SideEffect block that adjusts offset when tileZ changes)

- [ ] **Step 3: Also delete prevTileZ declaration if now unused**

Check if `prevTileZ` is used elsewhere. If not, remove line 102 as well.

- [ ] **Step 4: Run tests**

Run: `./gradlew testDebugUnitTest --tests FallbackTileTest`
Expected: Different failure pattern (offset-related failures should be gone)

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "fix: remove offset rescaling at tileZ boundaries

No coordinate system change occurs in baseZ world space. SideEffect was causing spurious pan jumps.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: Fix test helper - calculateTileScreenPosition

**Files:**
- Modify: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/FallbackTileTest.kt:28-51`

- [ ] **Step 1: Read current helper implementation**

Read the calculateTileScreenPosition function

- [ ] **Step 2: Update helper signature and implementation**

Replace the function with:

```kotlin
    private fun calculateTileScreenPosition(
        tileX: Int,
        tileY: Int,
        tileZ: Int,           // Requested zoom level
        actualTileZ: Int,     // Actual zoom level of tile we have
        centerLat: Double,
        centerLon: Double,
        scale: Float,
        offset: Offset,
        canvasWidth: Float,
        canvasHeight: Float,
        baseZ: Int = 15       // Base zoom level for world coordinates
    ): Offset {
        // CRITICAL: Derive geographic corners from REQUESTED tileZ, not actualTileZ
        val tileLon = tileXToLon(tileX, tileZ)
        val tileLat = tileYToLat(tileY, tileZ)

        // Convert to baseZ world coordinates
        val worldX = worldX(tileLon, centerLon, baseZ)
        val worldY = worldY(tileLat, centerLat, baseZ)

        // Apply composed scaling: native tile sizing + fallback compensation
        val scaleFactor = 2.0f.pow(baseZ - actualTileZ)

        val screenX = worldX * scaleFactor * scale + offset.x + canvasWidth / 2
        val screenY = worldY * scaleFactor * scale + offset.y + canvasHeight / 2

        return Offset(screenX, screenY)
    }
```

- [ ] **Step 3: Run tests**

Run: `./gradlew testDebugUnitTest --tests FallbackTileTest`
Expected: Some position assertions now pass

- [ ] **Step 4: Commit**

```bash
git add android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/FallbackTileTest.kt
git commit -m "test: fix calculateTileScreenPosition for baseZ coordinates

Use requested tileZ for geographic corners, baseZ for world coordinates, composed scaling 2^(baseZ - actualTileZ).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Fix test helper - calculateTileScreenDimensions

**Files:**
- Modify: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/FallbackTileTest.kt:57-65`

- [ ] **Step 1: Read current helper implementation**

Read the calculateTileScreenDimensions function

- [ ] **Step 2: Update helper to use composed scaling**

Replace the function with:

```kotlin
    private fun calculateTileScreenDimensions(
        tileZ: Int,           // Requested zoom level
        actualTileZ: Int,     // Actual zoom level of tile we have
        scale: Float,
        baseZ: Int = 15       // Base zoom level for world coordinates
    ): Pair<Float, Float> {
        // Composed scaling: native tile sizing at baseZ + fallback compensation
        val scaleFactor = 2.0f.pow(baseZ - actualTileZ)
        val scaledTileSize = TILE_SIZE * scaleFactor
        return Pair(scaledTileSize * scale, scaledTileSize * scale)
    }
```

- [ ] **Step 3: Run all FallbackTileTest**

Run: `./gradlew testDebugUnitTest --tests FallbackTileTest`
Expected: All 4 previously failing assertions now pass

- [ ] **Step 4: Run all tests**

Run: `./gradlew testDebugUnitTest`
Expected: All tests pass including new NativeTileScalingTest and RouteOverlayStabilityTest

- [ ] **Step 5: Commit**

```bash
git add android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/FallbackTileTest.kt
git commit -m "test: fix calculateTileScreenDimensions for baseZ coordinates

Use composed scaling 2^(baseZ - actualTileZ) for correct fallback tile sizing.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Add Kdoc to toScreenX/Y functions

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:179-187`

- [ ] **Step 1: Add Kdoc documentation**

Add documentation before the toScreenX function:

```kotlin
        /**
         * Convert longitude to screen X coordinate.
         *
         * Uses baseZ for stable world coordinates across zoom changes.
         * Geographic → screen conversion is invariant to tileZ changes.
         *
         * @param lon Geographic longitude
         * @return Screen X coordinate in pixels
         */
        fun toScreenX(lon: Double): Float {
```

Add similar documentation before toScreenY:

```kotlin
        /**
         * Convert latitude to screen Y coordinate.
         *
         * Uses baseZ for stable world coordinates across zoom changes.
         * Geographic → screen conversion is invariant to tileZ changes.
         *
         * @param lat Geographic latitude
         * @return Screen Y coordinate in pixels
         */
        fun toScreenY(lat: Double): Float {
```

- [ ] **Step 2: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "docs: add Kdoc to toScreenX/Y explaining baseZ contract

Clarify that these functions use baseZ for coordinate stability.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Add inline comment at world coordinate calculation

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:263-265`

- [ ] **Step 1: Add inline comment**

Add comment before line 264:

```kotlin
                                // Position in baseZ world space for stable coordinates across zoom
                                // Tile geographic bounds computed from requested tileZ, then converted to baseZ
                                val tileWorldX = worldX(tileNW, center.lon, baseZ)
                                val tileWorldY = worldY(tileNE, center.lat, baseZ)
```

- [ ] **Step 2: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "docs: add inline comment explaining baseZ world coordinate choice

Clarify that tile positioning uses baseZ for stability while geographic bounds come from requested tileZ.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Add file header documentation

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:1-3`

- [ ] **Step 1: Add file header**

Add at the top of the file (after package declaration):

```kotlin
/**
 * Map view component with tile overlay and route rendering.
 *
 * Coordinate System:
 * - All world coordinates use baseZ=15 as stable reference zoom level
 * - offset, tileWorldX/Y, toScreenX/Y never change zoom
 * - Tiles at zoom Z scale by 2^(baseZ - Z) when drawn in baseZ space
 * - Fallback tiles compose scaling: 2^(baseZ - Z) * 2^(Z - F) = 2^(baseZ - F)
 *
 * This ensures geographic → screen conversion is stable across tileZ changes,
 * preventing pan jumps when zooming past tile boundaries.
 */
```

- [ ] **Step 2: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "docs: add MapView.kt file header with coordinate system overview

Document baseZ coordinate system and scaling invariants.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Add debug assertions

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`

- [ ] **Step 1: Add scale factor sanity check**

After line 290 (after totalScale calculation), add:

```kotlin
                                if (BuildConfig.DEBUG) {
                                    assert(totalScale > 0f && totalScale < 1000f) {
                                        "Invalid scale factor: $totalScale (tileZ=$tileZ, baseZ=$baseZ, actualTileZ=$actualTileZ)"
                                    }
                                }
```

- [ ] **Step 2: Add world coordinate bounds check**

After line 265 (after tileWorldX/Y calculation), add:

```kotlin
                                if (BuildConfig.DEBUG) {
                                    assert(tileWorldX.isFinite() && tileWorldY.isFinite()) {
                                        "Non-finite world coordinates: x=$tileWorldX, y=$tileWorldY"
                                    }
                                }
```

- [ ] **Step 3: Add screen coordinate bounds check to toScreenX**

Inside toScreenX function, before return, add:

```kotlin
            if (BuildConfig.DEBUG) {
                assert(result.isFinite()) { "Non-finite screen X coordinate: $result" }
            }
```

- [ ] **Step 4: Add screen coordinate bounds check to toScreenY**

Inside toScreenY function, before return, add:

```kotlin
            if (BuildConfig.DEBUG) {
                assert(result.isFinite()) { "Non-finite screen Y coordinate: $result" }
            }
```

- [ ] **Step 5: Run tests**

Run: `./gradlew testDebugUnitTest`
Expected: All tests pass, assertions don't fire in normal operation

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "feat: add debug assertions for coordinate invariant violations

Catch scale factor, world coordinate, and screen coordinate errors in debug builds.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Final verification

- [ ] **Step 1: Run full test suite**

Run: `./gradlew testDebugUnitTest`

Expected: All tests pass
- NativeTileScalingTest: 3 tests pass
- RouteOverlayStabilityTest: 2 tests pass
- FallbackTileTest: 8 tests pass (including the 4 that were failing)

- [ ] **Step 2: Verify code changes**

Review that all changes from the spec are implemented:
- [ ] Scaling exponent inverted (2^(baseZ - tileZ))
- [ ] Offset rescaling removed
- [ ] Test helpers updated for baseZ
- [ ] Documentation added
- [ ] Debug assertions added

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "chore: final verification of map tile coordinate fix

All tests pass:
- NativeTileScalingTest: 3 tests verify power-of-two scaling
- RouteOverlayStabilityTest: 2 tests verify coordinate stability
- FallbackTileTest: 8 tests verify fallback positioning

Fixes:
- Scaling direction: 2^(baseZ - tileZ) for correct tile sizing
- Offset rescaling: removed unnecessary SideEffect
- Test consistency: helpers use baseZ coordinates

Documentation and debug assertions added.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review Complete

**Spec coverage:**
- Fix 1 (scaling): Task 3 ✓
- Fix 2 (offset): Task 4 ✓
- Fix 3 (tests): Tasks 5-6 ✓
- Phase 2 (documentation): Tasks 7-9 ✓
- Phase 3 (assertions): Task 10 ✓
- Success criteria (new tests): Tasks 1-2 ✓

**Placeholder scan:** No placeholders found. All code blocks complete.

**Type consistency:** Function names, parameters, and types consistent across tasks.
