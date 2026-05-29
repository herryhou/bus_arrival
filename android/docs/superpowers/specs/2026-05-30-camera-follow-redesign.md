# Camera Follow Redesign

## Problem Statement

Current camera follow implementation has multiple architectural issues:
- Redundant edge detection (duplicate code in 2 LaunchedEffects)
- Animation targets stale position (bus moves during 350ms animation)
- Race condition on follow toggle (uses stale currentSCm from closure)
- Complex state machine (3 LaunchedEffects with overlapping responsibilities)
- Fixed 100px threshold doesn't scale with screen size

Users report:
- Flickering "Follow" button when dragging
- Bus doesn't auto-pan when toggling Follow on with bus off-screen

## Requirements

1. **Toggle Follow ON**: Always animate bus to screen center, regardless of current position
2. **Follow enabled**: Track bus continuously, interpolate toward center when near edge
3. **User gesture**: Auto-disable Follow, stop animation immediately
4. **Smooth animation**: Ease-in-out, no jumps, tracks moving target

## Architecture

### Single LaunchedEffect

Replaces current 3-effect system with one unified effect.

**Dependencies**: `shouldFollow, currentSCm, scale, canvasSize, isUserInteracting`

**State**:
- `wasFollowingLastFrame`: Track if follow was enabled previously (detect enable transition)
- `targetOffset`: Where camera wants to be (null = no interpolation needed)
- `isUserInteracting`: User is currently gesturing

**Per-frame logic (60fps)**:
```
1. If !shouldFollow → exit
2. If isUserInteracting → skip frame
3. If !wasFollowingLastFrame AND shouldFollow → just enabled, set target
4. Else if bus near edge → set target = center bus
5. Else → target = null (stop interpolating)
6. If target exists → interpolate offset toward target
7. Update wasFollowingLastFrame = shouldFollow
```

### Edge Detection

**Ratio-based thresholds** (scale with screen size):
- Edge zone: outer 25% of screen width/height
- Center zone: inner 50% of screen width/height
- Transition zone: 25% buffer between edge and center

```kotlin
val edgeThreshold = size.width * 0.25f
val centerThreshold = size.width * 0.25f

val nearEdge = screenX < edgeThreshold ||
               screenX > size.width - edgeThreshold ||
               screenY < edgeThreshold ||
               screenY > size.height - edgeThreshold
```

### Interpolation Math

**Target calculation**:
```kotlin
val busWorldX = lonToPixelX(busLon, BASE_Z) - lonToPixelX(centerLon, BASE_Z)
val busWorldY = latToPixelY(busLat, BASE_Z) - latToPixelY(centerLat, BASE_Z)
targetOffset = Offset(-busWorldX * scale, -busWorldY * scale)
```

**Per-frame interpolation** (when target active):
```kotlin
val lerpFactor = 0.15f // 15% per frame → ~350ms to settle
val newOffset = currentOffset + (targetOffset - currentOffset) * lerpFactor
```

**Stop condition**:
```kotlin
val distance = abs(targetOffset - currentOffset)
if distance < 1f {
    currentOffset = targetOffset // Snap
    targetOffset = null // Stop
}
```

### User Gesture Handling

```kotlin
// In gesture detector
if ((isCameraFollowEnabled || replayState.cameraFollowEnabled) && !isUserInteracting) {
    isUserInteracting = true
    onToggleCameraFollow() // Auto-disable
}

// Reset after gesture ends
LaunchedEffect(isUserInteracting) {
    if (isUserInteracting) {
        delay(100L)
        isUserInteracting = false
    }
}
```

## Data Flow

### Initial Center-on-Enable
```
User clicks Follow button
→ shouldFollow changes (false → true)
→ LaunchedEffect restarts
→ wasFollowingLastFrame = false, shouldFollow = true
→ Set target = bus position
→ Animate until target reached
→ wasFollowingLastFrame = true (normal tracking)
```

### Continuous Edge Tracking
```
Bus moves toward edge
→ LaunchedEffect runs every frame (60fps)
→ Calculate bus screen position
→ nearEdge = true
→ Set target = bus position
→ Interpolate offset (15% per frame)
→ Bus exits edge zone
→ nearEdge = false
→ target = null
→ Stop interpolating
```

### User Gesture Interruption
```
User pans/zooms
→ detectTransformGestures fires
→ isUserInteracting = true
→ onToggleCameraFollow() → shouldFollow = false
→ LaunchedEffect exits
→ After 100ms: isUserInteracting = false
```

## Error Handling

- **No route data**: Exit LaunchedEffect early
- **Canvas size zero**: Exit early, wait for layout
- **Invalid GPS (0,0)**: Skip position calculation
- **Coroutine canceled**: Compose handles gracefully on effect restart

## Testing Strategy

### Unit Tests
1. Edge detection calculation (various screen positions)
2. Ratio-based thresholds (verify scaling)
3. Interpolation math (lerp converges to target)

### Integration Tests
1. Toggle follow ON with bus off-screen → animates to center
2. Toggle follow ON with bus on-screen → animates to center
3. Bus enters edge zone → camera follows smoothly
4. Bus exits edge zone → camera stops interpolating
5. User gesture → follow disables, animation stops
6. Rapid follow toggle → no crash, clean animation

### Manual Tests
1. Bus off-screen, toggle Follow → animates to center
2. Bus on-screen, toggle Follow → animates to center
3. Bus moving fast, Follow enabled → tracks near edge without jumping
4. User drags map → Follow turns off, no flicker
5. User zooms → Follow turns off, smooth stop

## Implementation Notes

- Remove `wasNearEdge` flag (no longer needed)
- Remove LaunchedEffect #3 (follow-toggle check) - handled by main effect
- Keep `isUserInteracting` flag and reset LaunchedEffect
- Change edgeThresholdPx from 100f to ratio-based (0.25f)
- Animation: continuous 60fps interpolation vs. fixed 350ms while loop
