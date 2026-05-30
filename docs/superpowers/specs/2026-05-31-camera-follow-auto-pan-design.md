# Camera Follow Auto-Pan Feature

**Date:** 2026-05-31
**Component:** Android App - MapView
**Status:** Design Approved

## Overview

Automatic map panning to keep the vehicle marker visible within the inner 80% of the viewport when the "Camera Follow" toggle is enabled. Triggers on GPS updates (live mode) and replay position changes.

## Requirements

- **Trigger condition:** Vehicle marker exits inner 80% of viewport (10% margin from edges)
- **Activation:** Both GPS (live) and simulator (replay) modes
- **Animation:** 300ms smooth tween using `animateOffsetAsState()`
- **Default state:** Global ON, persisted in preferences
- **User interaction:** Any user map transform gesture (pan or pinch zoom) auto-disables Follow

## Architecture

### Approach: Hybrid Separation

```
DetectionViewModel                MapView.kt
├── cameraFollowEnabled    ────> ├── Viewport check (10% margin)
├── toggleCameraFollow()          ├── Auto-pan trigger
└── disableCameraFollow()   <─────├── Transform gesture detection
                                    └── animateOffsetAsState() (300ms)
```

**State ownership:**
- ViewModel: `cameraFollowEnabled` (user intent, persisted)
- MapView: Viewport math, animation (rendering behavior)

## Components

### 1. DetectionViewModel

Add state and methods:

```kotlin
// State
private val _cameraFollowEnabled = MutableStateFlow(preferences.cameraFollowEnabled)
val cameraFollowEnabled: StateFlow<Boolean> = _cameraFollowEnabled.asStateFlow()

// Methods
fun toggleCameraFollow() {
    _cameraFollowEnabled.value = !_cameraFollowEnabled.value
    preferences.cameraFollowEnabled = _cameraFollowEnabled.value
}

fun disableCameraFollow() {
    _cameraFollowEnabled.value = false
    preferences.cameraFollowEnabled = false
}
```

### 2. MapView

#### Vehicle Position Computation (Add to MapView)

**Step 1:** Derive vehicle world/LatLon position (independent of offset):

```kotlin
// LatLon of the followed vehicle (GPS or replay marker) - no offset dependency
val followedVehicleLatLon by remember(routeData, currentSCm, gpsLat, gpsLon, replayState) {
    derivedStateOf {
        val route = routeData ?: return@derivedStateOf null

        // Priority: Replay marker > GPS marker
        if (replayState.traceFile != null && currentSCm >= 0) {
            // Replay mode: use interpolated route position (0 is valid start-of-route)
            val pos = route.interpolatePosition(currentSCm)
            if (pos != null) route.cmToLatLon(pos.first, pos.second) else null
        } else {
            // Live mode: use GPS position
            if (gpsLat == 0.0 || gpsLon == 0.0) null
            else LatLon(gpsLat, gpsLon)
        }
    }
}
```

**Step 2:** Compute screen position inside trigger effect with current offset:

```kotlin
LaunchedEffect(cameraFollowEnabled, followedVehicleLatLon, offset, scale, canvasSize.value) {
    // ... trigger logic computes screen pos here
}
```

#### Auto-pan Logic (LaunchedEffect + animateOffsetAsState)

Use `animateOffsetAsState` for smooth animation with LaunchedEffect for trigger logic:

```kotlin
val cameraFollowEnabled by viewModel.cameraFollowEnabled.collectAsState()

// Target offset for animation (null = no animation in progress)
var autoPanTarget by remember { mutableStateOf<Offset?>(null) }

// Animated offset that smoothly transitions to target
val animatedOffset by animateOffsetAsState(
    targetValue = autoPanTarget ?: offset,
    animationSpec = tween(durationMillis = 300, easing = EaseInOutCubic),
    label = "cameraFollow"
)

// Apply animated offset when follow is enabled AND animation is active
LaunchedEffect(animatedOffset, cameraFollowEnabled) {
    if (cameraFollowEnabled && autoPanTarget != null) {
        viewModel.updateMapState(scale, animatedOffset)
    }
}

// Cancel pending auto-pan when Follow is disabled via toggle or gesture
LaunchedEffect(cameraFollowEnabled) {
    if (!cameraFollowEnabled) {
        autoPanTarget = null
    }
}

// Trigger viewport check when vehicle moves (keys include offset for screen pos calculation)
LaunchedEffect(cameraFollowEnabled, followedVehicleLatLon, offset, canvasSize.value, scale) {
    if (!cameraFollowEnabled || autoPanTarget != null) return@LaunchedEffect
    val posLL = followedVehicleLatLon ?: return@LaunchedEffect
    val center = centerLatLon ?: return@LaunchedEffect
    val size = canvasSize.value
    if (size.width <= 0 || size.height <= 0) return@LaunchedEffect

    // Compute screen position with CURRENT offset (not stale)
    val worldX = lonToPixelX(posLL.lon, BASE_Z) - lonToPixelX(center.lon, BASE_Z)
    val worldY = latToPixelY(posLL.lat, BASE_Z) - latToPixelY(center.lat, BASE_Z)
    val pos = Offset(
        x = worldX * scale + offset.x + size.width / 2f,
        y = worldY * scale + offset.y + size.height / 2f
    )
    if (!pos.x.isFinite() || !pos.y.isFinite()) return@LaunchedEffect

    // Check 10% margin from edges
    val marginX = size.width * 0.1f
    val marginY = size.height * 0.1f

    if (pos.x < marginX || pos.x > size.width - marginX ||
        pos.y < marginY || pos.y > size.height - marginY) {
        // Vehicle outside safe zone: trigger animation
        val centerX = size.width / 2f
        val centerY = size.height / 2f
        autoPanTarget = Offset(
            x = offset.x + (centerX - pos.x),
            y = offset.y + (centerY - pos.y)
        )
    }
}

// Clear target when animation completes
LaunchedEffect(animatedOffset, autoPanTarget) {
    if (autoPanTarget != null && animatedOffset == autoPanTarget) {
        autoPanTarget = null
    }
}
```

#### Transform Gesture Integration

Modify existing `detectTransformGestures` block to cancel animation and disable follow for user-initiated map transforms:

```kotlin
detectTransformGestures { centroid, pan, zoom, _ ->
    if (cameraFollowEnabled || autoPanTarget != null) {
        autoPanTarget = null  // Cancel any in-progress auto-pan animation
        if (cameraFollowEnabled) {
            viewModel.disableCameraFollow()
        }
    }
    // ... existing pan/zoom logic
}
```

### 3. Follow Toggle UI

Replace existing stub (MapView.kt:641-668) with functional toggle:

```kotlin
Column(modifier = Modifier.align(Alignment.TopEnd).padding(16.dp)) {
    Box(
        modifier = Modifier
            .semantics { contentDescription = "Toggle camera follow" }
            .background(
                color = if (cameraFollowEnabled)
                    MaterialTheme.colorScheme.primaryContainer
                else
                    MaterialTheme.colorScheme.surfaceVariant,
                shape = MaterialTheme.shapes.large
            )
            .border(
                width = if (cameraFollowEnabled) 2.dp else 1.dp,
                color = if (cameraFollowEnabled)
                    MaterialTheme.colorScheme.primary
                else
                    MaterialTheme.colorScheme.outline,
                shape = MaterialTheme.shapes.large
            )
            .clickable { viewModel.toggleCameraFollow() }
            .padding(horizontal = 12.dp, vertical = 6.dp)
    ) {
        Row(
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically
        ) {
            if (cameraFollowEnabled) {
                Icon(
                    imageVector = Icons.Default.Check,
                    contentDescription = "Follow enabled",
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(16.dp)
                )
                Spacer(modifier = Modifier.width(4.dp))
            }
            Text(
                text = "Follow",
                style = MaterialTheme.typography.labelMedium,
                color = if (cameraFollowEnabled)
                    MaterialTheme.colorScheme.primary
                else
                    MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
```

### 4. DetectionPreferences

Add persistence:

```kotlin
var cameraFollowEnabled: Boolean
    get() = prefs.getBoolean(KEY_CAMERA_FOLLOW, true)
    set(value) = prefs.edit().putBoolean(KEY_CAMERA_FOLLOW, value).apply()

companion object {
    private const val KEY_CAMERA_FOLLOW = "camera_follow_enabled"
}
```

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| No GPS signal | `followedVehicleLatLon` is null → LaunchedEffect skips (no-op) |
| Canvas not ready | `canvasSize` is zero → LaunchedEffect skips (no-op) |
| Route not loaded | Toggle hidden, no auto-pan possible |
| User transform during animation | `autoPanTarget` cleared, animation LaunchedEffect guarded on `cameraFollowEnabled` |
| Replay scrubbing | Each seek triggers viewport check |
| Follow OFF | Zero overhead, LaunchedEffect returns early, `animatedOffset` falls back to current `offset` |
| Scale changes while Follow remains enabled | Margin recalculates in screen coords |

## Files to Modify

1. `DetectionViewModel.kt` - Add `cameraFollowEnabled` state, `toggleCameraFollow()`, `disableCameraFollow()`
2. `MapView.kt` - Add LaunchedEffect, animation, toggle UI, transform gesture integration
3. `DetectionPreferences.kt` - Add `cameraFollowEnabled` persistence
4. `CameraFollowViewportTest.kt` (NEW) - Viewport math unit tests

## Testing

### Unit Tests

**ViewModel:**
- `toggleCameraFollow()` flips state
- `disableCameraFollow()` sets false
- Initial state loads from preferences (default true)

**Viewport math** (`CameraFollowViewportTest.kt`):
- Vehicle at center → no auto-pan (inside safe zone)
- Vehicle at 5% margin → auto-pan triggers (OUTSIDE safe zone, < 10%)
- Vehicle at 15% margin → no auto-pan (inside safe zone, > 10%)
- Vehicle outside viewport → auto-pan centers it
- Scale changes → margin recalculates correctly

### Integration Tests (Manual)

**Basic behavior:**
1. GPS mode: Enable Follow, drive bus → verify auto-pan
2. Replay mode: Enable Follow, play trace → verify auto-pan
3. Toggle UI: Click Follow button → state changes, visual updates

**Cancellation and edge cases (high-risk):**
4. Pan during animation: Enable Follow, wait for auto-pan to start, then immediately pan → animation cancels, Follow disables, map stays at user-pan position
5. Pan after auto-pan completes: Manual pan → Follow disables, subsequent vehicle movement does NOT trigger auto-pan
6. Rapid vehicle movement: Enable Follow, bus moves quickly → auto-pan triggers on each viewport boundary crossing (not stuck)
7. Pinch-zoom during animation: Follow disables, pending animation cancels, user zoom/pan wins
8. Disable via toggle during animation: Click Follow button during auto-pan → animation cancels immediately

## Implementation Notes

- Zero regression risk: Feature is purely additive
- No changes to existing map rendering or GPS processing
- Animation uses standard Compose API (`animateOffsetAsState`)
- 300ms duration matches typical map gesture animations
