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
3. **User gesture**: Auto-disable active follow source (live or replay), stop animation immediately
4. **Smooth animation**: Exponential lerp (ease-out), no jumps, tracks moving target
5. **Position source**: Use route-interpolated position (from currentSCm) for both live and replay modes. Raw GPS only for bus marker rendering, not camera follow.

## Architecture

### Single LaunchedEffect

Replaces current 3-effect system with one unified effect.

**Dependencies**: `shouldFollow, routeData, centerLatLon, scale, canvasSize, isUserInteracting`

**Non-key dependencies** (read via `rememberUpdatedState` to prevent animation restart):
- `currentSCm`: Position updates every ~100ms, would cancel animation if key
- `offset`: Animation writes this every frame, would cause infinite restart loop if key

**Access pattern**:
```kotlin
val currentSCm by rememberUpdatedState(viewModel.currentSCm)
val offset by rememberUpdatedState(viewModel.mapOffset.collectAsState().value)
```
Both values refresh inside loop without triggering effect restart.

**Route data handling**: Route changes trigger effect restart (via dependencies), ensuring position calculations use current route geometry and center point.

**State**:
- `wasFollowingLastFrame`: Track if follow was enabled previously (detect enable transition)
- `targetOffset`: Where camera wants to be (null = no interpolation needed)
- `isUserInteracting`: User is currently gesturing

**Per-frame logic (60fps)**:
```
1. If !shouldFollow → exit, wasFollowingLastFrame = false
2. If isUserInteracting → delay(16L), continue loop (don't busy-loop)
3. Get current bus position from routeData.interpolatePosition(currentSCm)
4. If position == null → delay(16L), continue loop (route unavailable, don't busy-loop)
5. If !wasFollowingLastFrame AND shouldFollow → just enabled, set target = center bus
6. Else if nearEdge → set target = center bus (recompute every frame)
7. Else if inCenter → target = null (stop interpolating, allow comfortable center zone)

8. IDLE GUARD: If target == null → delay(100L), continue loop
   (Prevent unnecessary recomposition when camera settled)

9. If target exists → interpolate offset toward target
   CRITICAL: When target active (near edge or just enabled), recompute target every frame
   from current currentSCm. This tracks moving bus even in transition zone.

10. MOVEMENT THRESHOLD: Only update state if movement >= 0.5px
    if ((newOffset - currentOffset).getDistance() >= 0.5f) {
        viewModel.updateMapState(scale, newOffset)
    }
    (Prevent tile loading churn from micro-movements)

11. STOP CONDITION: When reached target, update state one final time
    if distance < 1px {
        viewModel.updateMapState(scale, targetOffset) // Final snap
        targetOffset = null
    }

12. ACTIVE FRAME PACE: delay(16L) to maintain ~60fps
    (Prevent loop running faster than frame rate)

13. Update wasFollowingLastFrame = shouldFollow
```

### Edge Detection

**Ratio-based thresholds** (scale with screen size):
- **Edge zone**: outer 25% of screen (start following when bus here)
- **Center zone**: inner 40% of screen (stop following when bus here)
- **Transition gap**: 10% total horizontal/vertical dead band (5% per edge) prevents oscillation

**Hysteresis to prevent oscillation:**
- Follow STARTS when bus enters edge zone (outside 25% from edges)
- Follow STOPS when bus enters center zone (inside 40% from edges)
- 10% dead band prevents rapid toggle at boundary

```kotlin
// Edge zone: outer 25%
val edgeThresholdX = size.width * 0.25f
val edgeThresholdY = size.height * 0.25f

// Center zone: inner 40%
val centerThresholdX = size.width * 0.30f  // 30% from left/right = 40% center
val centerThresholdY = size.height * 0.30f // 30% from top/bottom = 40% center

val nearEdge = screenX < edgeThresholdX ||
               screenX > size.width - edgeThresholdX ||
               screenY < edgeThresholdY ||
               screenY > size.height - edgeThresholdY

val inCenter = screenX > centerThresholdX &&
               screenX < size.width - centerThresholdX &&
               screenY > centerThresholdY &&
               screenY < size.height - centerThresholdY
```

### Interpolation Math

**Position source**: Always use route-interpolated position from `currentSCm` (works for both live and replay modes).
```kotlin
val pos = routeData.interpolatePosition(currentSCm)
if (pos != null) {
    val ll = routeData.cmToLatLon(pos.first, pos.second)
    // Calculate world coordinates from ll.lon, ll.lat
}
```

**Target calculation**:
```kotlin
val busWorldX = lonToPixelX(ll.lon, BASE_Z) - lonToPixelX(centerLon, BASE_Z)
val busWorldY = latToPixelY(ll.lat, BASE_Z) - latToPixelY(centerLat, BASE_Z)
targetOffset = Offset(-busWorldX * scale, -busWorldY * scale)
```

**Per-frame interpolation** (exponential lerp, ease-out):
```kotlin
val lerpFactor = 0.15f // 15% per frame → exponential decay, ~350ms to settle
val newOffset = currentOffset + (targetOffset - currentOffset) * lerpFactor
```
This produces ease-out motion: fast initially, slowing as it approaches target. Each frame covers 15% of remaining distance.

**Target recomputation** (critical for tracking moving bus):
```kotlin
// When actively tracking (near edge or just enabled), recompute target every frame
val pos = routeData.interpolatePosition(currentSCm)
val ll = routeData.cmToLatLon(pos.first, pos.second)
val targetOffset = Offset(-busWorldX * scale, -busWorldY * scale)
// This ensures camera tracks bus even in transition zone
```

**Stop condition**:
```kotlin
val distance = abs(targetOffset - currentOffset)
if distance < 1f {
    viewModel.updateMapState(scale, targetOffset) // Final snap to state
    targetOffset = null // Stop tracking
}
```

**Movement threshold for state update**:
```kotlin
// Only trigger recomposition if movement is meaningful
val movementDelta = (newOffset - currentOffset).getDistance()
if (movementDelta >= 0.5f) {
    viewModel.updateMapState(scale, newOffset)
}
```

### User Gesture Handling

**API requirement**: MapView needs separate callbacks for toggle (UI button) and disable (gesture):
```kotlin
// MapView signature changes
fun MapView(
    // ... existing params
    onToggleCameraFollow: () -> Unit = {},       // KEEP: for Follow button click
    onDisableLiveFollow: () -> Unit = {},        // NEW: gesture disables live follow
    onDisableReplayFollow: () -> Unit = {},     // NEW: gesture disables replay follow
)
```

**Gesture detector logic**:
```kotlin
// In gesture detector
if (!isUserInteracting) {
    isUserInteracting = true
    // Disable active follow sources (both may be active)
    if (isCameraFollowEnabled) {
        onDisableLiveFollow()
    }
    if (replayState.cameraFollowEnabled) {
        onDisableReplayFollow()
    }
}
```

**Follow button (unchanged)**:
```kotlin
// Top-right Follow button click handler
onClick = { onToggleCameraFollow() }
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
→ Enters edge zone (nearEdge = true)
→ Set target = bus position
→ Interpolate offset (15% per frame)
→ Bus exits edge zone, enters center zone (inCenter = true)
→ target = null
→ Stop interpolating (comfortable center zone prevents oscillation)
```

### User Gesture Interruption
```
User pans/zooms
→ detectTransformGestures fires
→ isUserInteracting = true
→ Check active follow source:
  - If live follow active: onDisableLiveFollow()
  - If replay follow active: onDisableReplayFollow()
→ shouldFollow = false
→ LaunchedEffect exits
→ After 100ms: isUserInteracting = false
```

## Error Handling

- **No route data**: Exit LaunchedEffect early
- **Canvas size zero**: Exit early, wait for layout
- **Invalid route position**: `interpolatePosition(currentSCm)` returns null → skip frame, continue loop
- **Coroutine canceled**: Compose handles gracefully on effect restart

## Performance Considerations

**Why 60fps loop is safe:**
- Math (coordinate transforms, edge checks, lerp) is cheap
- Expensive part is `viewModel.updateMapState()` → triggers recomposition, tile requests

**Two guards prevent unnecessary churn:**

1. **Idle guard**: When `targetOffset == null` (bus centered, no animation needed):
   ```kotlin
   if (targetOffset == null) {
       delay(100L) // Don't spin at 60fps doing nothing
       continue
   }
   ```

2. **Movement threshold**: Only update state when movement >= 0.5px:
   ```kotlin
   if ((newOffset - currentOffset).getDistance() >= 0.5f) {
       viewModel.updateMapState(scale, newOffset)
   }
   ```

**Result**: Loop only calls `updateMapState` when actively animating toward target. When bus centered, loop idles (100ms delay) without triggering recomposition.

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

- Remove `wasNearEdge` flag (no longer needed, use `wasFollowingLastFrame`)
- Remove LaunchedEffect #3 (follow-toggle check) - handled by main effect's transition detection
- Keep `isUserInteracting` flag and reset LaunchedEffect
- Change edgeThresholdPx from 100f to ratio-based (0.25f edge, 0.30f center for X and Y separately)
- Animation: continuous 60fps interpolation with `rememberUpdatedState` for `currentSCm` and `offset`
- API change: Keep `onToggleCameraFollow()` (for Follow button), add `onDisableLiveFollow()` and `onDisableReplayFollow()` (for gestures)
- Always use `routeData.interpolatePosition(currentSCm)` for camera follow target
- Raw GPS (gpsLat/gpsLon) only for bus marker rendering, never for camera follow
