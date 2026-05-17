# Map Tile Viewport Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix intermittent white map viewports by requesting and retaining every map tile that intersects the scaled viewport.

**Architecture:** Keep the existing Compose map pipeline. Make the tile coverage calculation scale-aware, then keep memory pressure inside the existing tile cache limit so visible tiles are not evicted immediately after loading.

**Tech Stack:** Kotlin, Android Compose Canvas, JUnit unit tests, existing `TileCache` and `DetectionViewModel` memory tile cache.

---

## Systematic Debugging Summary

**Observed symptom:** Sometimes the map viewport is pure white, especially when zoomed or panned. A tile that intersects even a small part of the viewport should be drawn.

**Root cause hypothesis:** `computeVisibleTileRange()` calculates viewport coverage in screen pixels, but tiles are drawn in base world pixels under a `scale` transform. At scale below `1f`, the viewport covers more world pixels than the helper accounts for, so the app requests too few tiles. The large request fallback can also exceed `MAX_MEMORY_TILES = 160`, causing nearest tiles to be evicted first due insertion order.

**Evidence from current code:**
- `MapView.kt:300-303` draws tiles inside `translate(...); scale(scaleX = scale, scaleY = scale)`.
- `MapView.kt:186-193` computes `drawTileRange` without passing `scale`.
- `MapView.kt:667-675` uses `canvasHalfMax / tileWorldSize`, missing `/ scale`.
- `MapView.kt:201` caps fetch range at `MAX_FETCH_RANGE = 12`, which can request `25 * 25 = 625` tiles.
- `DetectionViewModel.kt:33` caps memory cache at `160`.
- `DetectionViewModel.kt:232-236` trims from insertion order, while `MapView.kt:620` inserts nearest tiles first, so nearest tiles are evicted first on oversized loads.

**Success criteria:**
- A unit test fails before the fix showing scale `0.1f` requires a larger tile range than the current implementation returns.
- `computeVisibleTileRange()` accounts for scale and covers the viewport in world space.
- Fetch range is capped so a single load cannot exceed the memory tile cache in normal operation.
- Existing Android unit tests pass.

## Files

- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`
  - Add scale input to `computeVisibleTileRange()`.
  - Pass current `scale` from `MapView`.
  - Replace `MAX_FETCH_RANGE = 12` with a limit compatible with the memory cache.
- Modify: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/MapViewportRequestTest.kt`
  - Update existing tests for the new helper signature.
  - Add regression tests for zoomed-out viewport coverage and request-window size.

## Task 1: Prove Scaled Viewport Coverage Failure

**Files:**
- Modify: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/MapViewportRequestTest.kt`

- [ ] **Step 1: Add failing scale-aware tests**

Add these imports and tests to `MapViewportRequestTest`:

```kotlin
import kotlin.math.pow
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

class MapViewportRequestTest {
    // keep existing tests

    @Test
    fun visibleTileRangeAccountsForZoomedOutScale() {
        val range =
                computeVisibleTileRange(
                        zoom = 15,
                        canvasWidth = 1000f,
                        canvasHeight = 1000f,
                        scale = 0.1f
                )

        assertEquals(
                21,
                range,
                "At 0.1x, 1000px viewport covers 10000 world px, so z15 needs +/-21 tiles"
        )
    }

    @Test
    fun visibleTileRangeUsesScaledViewportWorldPixels() {
        val zoom = 16
        val canvasWidth = 1000f
        val canvasHeight = 800f
        val scale = 0.5f
        val range =
                computeVisibleTileRange(
                        zoom = zoom,
                        canvasWidth = canvasWidth,
                        canvasHeight = canvasHeight,
                        scale = scale
                )

        val tileWorldSize = 256f * 2.0f.pow(15 - zoom)
        val viewportHalfWorld = maxOf(canvasWidth, canvasHeight) / 2f / scale

        assertTrue(
                range * tileWorldSize >= viewportHalfWorld,
                "Tile range must cover the scaled viewport in world pixels"
        )
    }
}
```

- [ ] **Step 2: Run the focused test and confirm failure**

Run from `android/`:

```bash
rtk proxy ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.MapViewportRequestTest
```

Expected before implementation: compile failure because `computeVisibleTileRange()` has no `scale` parameter, or assertion failure if the signature was already changed without fixing the math.

## Task 2: Make Tile Range Scale-Aware

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`
- Modify: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/MapViewportRequestTest.kt`

- [ ] **Step 1: Pass `scale` into the derived tile range**

Change the `drawTileRange` block in `MapView.kt` from:

```kotlin
val drawTileRange by remember(requestedTileZ, canvasSize.value) {
    derivedStateOf {
        computeVisibleTileRange(
                zoom = requestedTileZ,
                canvasWidth = canvasSize.value.width.toFloat(),
                canvasHeight = canvasSize.value.height.toFloat()
        )
    }
}
```

to:

```kotlin
val drawTileRange by remember(requestedTileZ, canvasSize.value, scale) {
    derivedStateOf {
        computeVisibleTileRange(
                zoom = requestedTileZ,
                canvasWidth = canvasSize.value.width.toFloat(),
                canvasHeight = canvasSize.value.height.toFloat(),
                scale = scale
        )
    }
}
```

- [ ] **Step 2: Replace the helper implementation**

Change `computeVisibleTileRange()` in `MapView.kt` to:

```kotlin
internal fun computeVisibleTileRange(
        zoom: Int,
        canvasWidth: Float,
        canvasHeight: Float,
        scale: Float
): Int {
    if (canvasWidth <= 0f || canvasHeight <= 0f || scale <= 0f) {
        return 2
    }

    val tileWorldSize = 256f * 2.0f.pow(BASE_Z - zoom)
    val viewportHalfWorld = maxOf(canvasWidth, canvasHeight) / 2f / scale
    return kotlin.math.ceil(viewportHalfWorld / tileWorldSize).toInt() + 1
}
```

Why this is the minimal fix: drawing applies `scale` to world coordinates, so coverage must convert screen half-size back to world half-size by dividing by `scale`. `ceil(...) + 1` includes partially visible edge tiles.

- [ ] **Step 3: Update the existing test call sites**

Change existing calls in `MapViewportRequestTest.kt`:

```kotlin
val z15Range = computeVisibleTileRange(zoom = 15, canvasWidth = 1000f, canvasHeight = 1000f)
val z16Range = computeVisibleTileRange(zoom = 16, canvasWidth = 1000f, canvasHeight = 1000f)
```

to:

```kotlin
val z15Range =
        computeVisibleTileRange(zoom = 15, canvasWidth = 1000f, canvasHeight = 1000f, scale = 1f)
val z16Range =
        computeVisibleTileRange(zoom = 16, canvasWidth = 1000f, canvasHeight = 1000f, scale = 1f)
```

Also update the expected z15 value from `3` to `3` only if the new formula still returns `3`; with `ceil(500 / 256) + 1`, it returns `3`.

- [ ] **Step 4: Run the focused test**

Run from `android/`:

```bash
rtk proxy ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.MapViewportRequestTest
```

Expected: all tests in `MapViewportRequestTest` pass.

## Task 3: Prevent One Load From Evicting Visible Tiles

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`
- Modify: `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/MapViewportRequestTest.kt`

- [ ] **Step 1: Reduce fetch cap to fit the memory cache**

Change:

```kotlin
private const val MAX_FETCH_RANGE = 12
```

to:

```kotlin
private const val MAX_FETCH_RANGE = 5
```

Rationale: range `5` requests `11 * 11 = 121` tiles, which fits under `MAX_MEMORY_TILES = 160`. This keeps one load from evicting its own visible center tiles. This is a conservative stopgap; do not refactor `DetectionViewModel` cache eviction unless this still leaves a reproducible white viewport.

- [ ] **Step 2: Add a request-size regression test**

Add this helper and test to `MapViewportRequestTest.kt`:

```kotlin
private fun tileWindowSize(range: Int): Int {
    val side = range * 2 + 1
    return side * side
}

@Test
fun maxFetchRangeFitsMemoryTileCache() {
    val maxFetchRange = 5

    assertTrue(
            tileWindowSize(maxFetchRange) <= 160,
            "One tile load should fit within DetectionViewModel.MAX_MEMORY_TILES"
    )
}
```

- [ ] **Step 3: Run the focused tests**

Run from `android/`:

```bash
rtk proxy ./gradlew testDebugUnitTest --tests com.busarrival.app.presentation.ui.detection.components.MapViewportRequestTest
```

Expected: all tests pass.

## Task 4: Verify Broader Android Tests

**Files:**
- No code changes.

- [ ] **Step 1: Run all debug unit tests**

Run from `android/`:

```bash
rtk proxy ./gradlew testDebugUnitTest
```

Expected: build succeeds and unit tests pass.

- [ ] **Step 2: Inspect final diff**

Run from repo root:

```bash
rtk git diff -- android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/MapViewportRequestTest.kt
```

Expected:
- `computeVisibleTileRange()` takes `scale`.
- `MapView` passes current `scale`.
- `MAX_FETCH_RANGE` is `5`.
- Tests cover scale-aware range and memory-sized request window.

## Residual Risk

- This plan fixes the coverage calculation and immediate self-eviction risk. It does not redesign the memory cache eviction policy.
- If white viewports remain after this fix, the next root-cause pass should instrument which visible tile keys are computed, requested, loaded, retained, and drawn for one failing viewport.

