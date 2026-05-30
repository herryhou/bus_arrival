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
- **User interaction:** Any pan gesture auto-disables Follow

## Architecture

### Approach: Hybrid Separation

```
DetectionViewModel                MapView.kt
├── cameraFollowEnabled    ────> ├── Viewport check (10% margin)
├── toggleCameraFollow()          ├── Auto-pan trigger
└── disableCameraFollow()   <─────├── Pan gesture detection
                                    └── animateToAsState() (300ms)
```

**State ownership:**
- ViewModel: `cameraFollowEnabled` (user intent, persisted)
- MapView: Viewport math, animation (rendering behavior)

## Components

### 1. DetectionViewModel

Add state and methods:

```kotlin
// State
private val _cameraFollowEnabled = MutableStateFlow(preferences.cameraFollowEnabled ?: true)
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

#### Auto-pan Logic (LaunchedEffect)

```kotlin
val cameraFollowEnabled by viewModel.cameraFollowEnabled.collectAsState()

// Animated offset target
val targetOffset = remember { mutableStateOf(offset) }
val animatedOffset by animateOffsetAsState(
    targetValue = targetOffset.value,
    animationSpec = tween(durationMillis = 300)
)

LaunchedEffect(busScreenPosition, cameraFollowEnabled, canvasSize.value) {
    if (!cameraFollowEnabled) return@LaunchedEffect
    val pos = busScreenPosition ?: return@LaunchedEffect
    val size = canvasSize.value
    if (size.width <= 0 || size.height <= 0) return@LaunchedEffect

    // Check 10% margin from edges
    val marginX = size.width * 0.1f
    val marginY = size.height * 0.1f

    if (pos.x < marginX || pos.x > size.width - marginX ||
        pos.y < marginY || pos.y > size.height - marginY) {
        // Vehicle outside safe zone: calculate offset to center
        val targetWorldX = (size.width / 2f - pos.x) / scale
        val targetWorldY = (size.height / 2f - pos.y) / scale
        targetOffset.value = Offset(targetWorldX, targetWorldY)
        viewModel.updateMapState(scale, targetOffset.value)
    }
}
```

#### Pan Gesture Integration

Modify existing `detectTransformGestures` block:

```kotlin
detectTransformGestures { centroid, pan, zoom, _ ->
    if (cameraFollowEnabled) {
        viewModel.disableCameraFollow()
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
var cameraFollowEnabled: Boolean?
    get() = prefs.getBoolean(KEY_CAMERA_FOLLOW, true)
    set(value) = prefs.edit().putBoolean(KEY_CAMERA_FOLLOW, value).apply()

companion object {
    private const val KEY_CAMERA_FOLLOW = "camera_follow_enabled"
}
```

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| No GPS signal | `busScreenPosition` is null → LaunchedEffect skips (no-op) |
| Canvas not ready | `canvasSize` is zero → LaunchedEffect skips (no-op) |
| Route not loaded | Toggle hidden, no auto-pan possible |
| Pan during animation | Animation cancels, Follow disables |
| Replay scrubbing | Each seek triggers viewport check |
| Follow OFF | Zero overhead, LaunchedEffect returns early |
| Scale changes | Margin recalculates in screen coords |

## Files to Modify

1. `DetectionViewModel.kt` - Add `cameraFollowEnabled` state, `toggleCameraFollow()`, `disableCameraFollow()`
2. `MapView.kt` - Add LaunchedEffect, animation, toggle UI, pan gesture integration
3. `DetectionPreferences.kt` - Add `cameraFollowEnabled` persistence
4. `CameraFollowViewportTest.kt` (NEW) - Viewport math unit tests

## Testing

### Unit Tests

**ViewModel:**
- `toggleCameraFollow()` flips state
- `disableCameraFollow()` sets false
- Initial state loads from preferences (default true)

**Viewport math** (`CameraFollowViewportTest.kt`):
- Vehicle at center → no auto-pan
- Vehicle at 5% margin → no auto-pan
- Vehicle at 15% margin → auto-pan triggers
- Vehicle outside viewport → auto-pan centers it
- Scale changes → margin recalculates correctly

### Integration Tests (Manual)

1. GPS mode: Enable Follow, drive bus → verify auto-pan
2. Replay mode: Enable Follow, play trace → verify auto-pan
3. Pan gesture: Auto-pan active, user pans → Follow disables
4. Toggle UI: Click Follow button → state changes, visual updates

## Implementation Notes

- Zero regression risk: Feature is purely additive
- No changes to existing map rendering or GPS processing
- Animation uses standard Compose API (`animateOffsetAsState`)
- 300ms duration matches typical map gesture animations
