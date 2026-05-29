# Android Map UI Changes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add camera follow toggle, double vehicle heading arrow size (green), and convert 50m stop circle to semi-transparent filled disc

**Architecture:** Three isolated UI changes in MapView/VehicleHeadingMarker components using existing state management

**Tech Stack:** Jetpack Compose, Kotlin, Material3 icons

---

## File Structure

**Files to modify:**
1. `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarker.kt` - Arrow size + color
2. `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt` - Stop disc + toggle button

**No new files created** - all changes inline to existing components

---

## Task 1: Double Vehicle Heading Arrow Size (32.dp → 64.dp)

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarker.kt:23`

- [ ] **Step 1: Update marker size constant**

Change line 23 from:
```kotlin
internal val vehicleHeadingMarkerSize = 32.dp
```
to:
```kotlin
internal val vehicleHeadingMarkerSize = 64.dp
```

- [ ] **Step 2: Build to verify compiles**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 3: Run app to verify size increase**

Launch app, observe vehicle heading arrow is now 2x larger
Expected: Arrow appears significantly larger on map

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarker.kt
git commit -m "Double vehicle heading arrow size (32.dp -> 64.dp)"
```

---

## Task 2: Change Vehicle Heading Arrow Color to Green

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarker.kt:48-61`

- [ ] **Step 1: Read current arrow draw code**

Find the `rotate()` block that draws the arrow shaft/head (lines 48-61). Note the `brush` parameter uses `Color.Yellow`.

- [ ] **Step 2: Replace yellow with green**

Locate line ~55 with `brush = Brush.verticalGradient(...)`. Change from:
```kotlin
brush = Brush.verticalGradient(
    colors = listOf(
        Color.Yellow,
        Color.Yellow.copy(alpha = 0.8f)
    )
)
```
to:
```kotlin
brush = Brush.verticalGradient(
    colors = listOf(
        Color(0xFF2E7D32),  // Material Green 800
        Color(0xFF2E7D32).copy(alpha = 0.8f)
    )
)
```

- [ ] **Step 3: Build to verify compiles**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Run app to verify green color**

Launch app, observe arrow is now green (not yellow)
Expected: Arrow matches "live mode" green color scheme

- [ ] **Step 5: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarker.kt
git commit -m "Change vehicle heading arrow color to green (Material Green 800)"
```

---

## Task 3: Convert 50m Stop Circle to Semi-Transparent Filled Disc

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:529-534`

- [ ] **Step 1: Read current stop circle code**

Find the `drawCircle()` call inside the `routeData.stops.forEach` loop (lines ~529-534):
```kotlin
drawCircle(
    color = Color.Red.copy(alpha = 0.6f),
    radius = refRadiusPx,
    center = Offset(px, py),
    style = Stroke(width = 2f)
)
```

- [ ] **Step 2: Remove Stroke style, reduce alpha**

Replace the entire `drawCircle()` call with:
```kotlin
drawCircle(
    color = Color.Red.copy(alpha = 0.3f),
    radius = refRadiusPx,
    center = Offset(px, py)
)
```

Changes:
- Removed `style = Stroke(width = 2f)` (defaults to Fill)
- Changed `alpha = 0.6f` to `alpha = 0.3f` (softer fill)

- [ ] **Step 3: Build to verify compiles**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 4: Run app to verify filled disc**

Launch app, observe 50m stop circles are now filled (not outlined)
Expected: Semi-transparent red discs over stops, route path visible underneath

- [ ] **Step 5: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git commit -m "Convert 50m stop circle to semi-transparent filled disc"
```

---

## Task 4: Add Camera Follow Toggle Button (Top-Right)

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt:596-647`

This task adds an IconButton in the top-right corner of the map, mirroring the info button (top-left).

- [ ] **Step 1: Add MyLocation icon import**

Add to existing imports section (after line 31, where other icons are imported):
```kotlin
import androidx.compose.material.icons.filled.MyLocation
```

- [ ] **Step 2: Add toggle callback parameter**

Add new parameter to `MapView` function signature (after `gpsBearing: Float?`, line ~122):
```kotlin
onToggleCameraFollow: () -> Unit = {},
```

- [ ] **Step 3: Add camera follow toggle button**

Insert this code block **after** the debug info Column (after line 646, inside the main Box):

```kotlin
// Camera follow toggle (top-right, mirrors info button top-left)
Column(
    modifier = Modifier.align(Alignment.TopEnd).padding(16.dp),
    horizontalArrangement = Arrangement.End
) {
    IconButton(
        onClick = onToggleCameraFollow,
        modifier = Modifier
            .semantics { contentDescription = "Toggle camera follow" }
    ) {
        Icon(
            imageVector = Icons.Default.MyLocation,
            contentDescription = "Toggle camera follow",
            tint = if (isCameraFollowEnabled) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            }
        )
    }
}
```

This places the button at top-right with appropriate tint color based on state.

- [ ] **Step 4: Update DetectionScreen to pass toggle callback**

Open `app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt` and find the `MapView()` call (line ~100).

Add the callback parameter:
```kotlin
MapView(
    // ... existing parameters ...
    onToggleCameraFollow = { viewModel.toggleCameraFollow() }
)
```

- [ ] **Step 5: Build to verify compiles**

Run: `./gradlew compileDebugKotlin`
Expected: BUILD SUCCESSFUL

- [ ] **Step 6: Run app to verify toggle works**

Launch app, test:
1. Tap top-right icon → camera follow state toggles
2. Icon tint changes (primary when on, onSurfaceVariant when off)
3. Map follows/unfollows position correctly

Expected: Toggle button functional, state persists, map behavior matches

- [ ] **Step 7: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/DetectionScreen.kt
git commit -m "Add camera follow toggle button (top-right of map)"
```

---

## Task 5: Update VehicleHeadingMarkerTest for New Size

**Files:**
- Modify: `app/src/test/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarkerTest.kt:19`

- [ ] **Step 1: Read current test assertion**

Line 19 asserts:
```kotlin
assertEquals(vehicleHeadingMarkerSize / 2f, vehicleHeadingIconSize)
```
This expects icon = marker/2 (still valid after doubling).

- [ ] **Step 2: Update test to verify new size**

Change line 19 to explicitly verify the new 64.dp size:
```kotlin
assertEquals(64.dp.value, vehicleHeadingMarkerSize.value)
assertEquals(32.dp.value, vehicleHeadingIconSize.value)
```

- [ ] **Step 2.5: Update hardcoded test values**

The positioning tests use hardcoded 32f/16f values. Update lines 25-26:
```kotlin
val markerPx = 64f  // was 32f
val iconPx = 32f     // was 16f
```

And update the expected results (lines 28-50). When marker doubles, center doubles (16→32) and iconHalf doubles (8→16):

- Line 29: `Offset(68f, 84f)` (was `84f, 92f`)
  - calc: 100 - 32 + 0, 100 - 32 + 16
- Line 32: `Offset(52f, 68f)` (was `76f, 84f`)
  - calc: 100 - 32 - 16, 100 - 32 + 0
- Line 36: `Offset(68f, 52f)` (was `84f, 76f`)
  - calc: 100 - 32 + 0, 100 - 32 - 16
- Line 40: `Offset(84f, 68f)` (was `92f, 84f`)
  - calc: 100 - 32 + 16, 100 - 32 + 0
- Line 49: `Offset(68f, 68f)` (was `84f, 84f`)
  - calc: 100 - 32, 100 - 32 (null bearing)

- [ ] **Step 3: Run test to verify passes**

Run: `./gradlew test --tests VehicleHeadingMarkerTest`
Expected: PASSED

- [ ] **Step 4: Commit**

```bash
git add app/src/test/java/com/busarrival/app/presentation/ui/detection/components/VehicleHeadingMarkerTest.kt
git commit -m "Update VehicleHeadingMarkerTest for 64.dp marker size"
```

---

## Task 6: Final Verification & Integration Test

**Files:**
- No file changes - manual verification

- [ ] **Step 1: Full build and install**

Run: `./gradlew assembleDebug installDebug`
Expected: BUILD SUCCESSFUL, app installed

- [ ] **Step 2: Manual test checklist**

Test each change:
1. [ ] Camera follow toggle visible top-right, toggles on tap
2. [ ] Vehicle heading arrow is 2x larger than before
3. [ ] Arrow color is green (not yellow)
4. [ ] 50m stop circles are filled (not outlined)
5. [ ] Stop disc transparency allows route to show through
6. [ ] Map pan/zoom still works
7. [ ] Tiles load correctly
8. [ ] No crashes when rotating screen

- [ ] **Step 3: Regression test**

Run existing tests:
```bash
./gradlew test
```
Expected: All tests pass

- [ ] **Step 4: Final commit if needed**

If any minor tweaks needed:
```bash
git add -A
git commit -m "Fix minor issues from Android Map UI changes"
```

---

## Testing Strategy

- **Unit tests:** VehicleHeadingMarkerTest verifies size calculations
- **Manual tests:** Visual verification of all three changes
- **Regression tests:** Existing test suite ensures no breakage

## Notes

- Camera follow state already exists in `DetectionViewModel.isCameraFollowEnabled` (line 719)
- Toggle function `toggleCameraFollow()` already exists (line 330)
- No new state needed, only UI exposure
- All changes are additive or inline replacements — no breaking changes
