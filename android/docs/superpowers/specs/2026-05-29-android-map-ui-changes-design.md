# Android Map UI Changes Design

**Date:** 2026-05-29
**Status:** Approved
**Component:** MapView.kt, VehicleHeadingMarker.kt

## Overview

Three targeted UI changes to the Android bus arrival map view:
1. Camera follow toggle button (user can on/off it)
2. Vehicle heading arrow: double in size (32.dp → 64.dp), green color
3. 50m stop reference circle → semi-transparent filled disc

## Approach

**Option A - Minimal Inline Changes** (chosen for smallest scope, fastest implementation)

## 1. Camera Follow Toggle

### Location
Top-right corner of map canvas, aligned with existing info button (top-left)

### UI Specification
- **Component:** `IconButton`
- **Position:** `Modifier.align(Alignment.TopEnd).padding(16.dp)`
- **Icon:**
  - Enabled: `Icons.Default.MyLocation` (filled crosshair)
  - Disabled: `Icons.Default.MyLocation` with reduced alpha or different tint
- **Size:** 48.dp
- **Background:**
  - Enabled: White circle with shadow (to match info button style)
  - Disabled: Transparent or muted gray
- **Accessibility:** `contentDescription = "Toggle camera follow"`

### Behavior
- Toggles `isCameraFollowEnabled` state via existing `viewModel.toggleCameraFollow()`
- State already managed in `DetectionViewModel.kt` line 330
- No new state needed

### Files
- `MapView.kt`: Add IconButton in top-right Box alignment (near line 597, below debug info section)

## 2. Vehicle Heading Arrow Size & Color

### Current State
- `vehicleHeadingMarkerSize = 32.dp` (VehicleHeadingMarker.kt line 23)
- Arrow color: yellow (`Color.Yellow`)

### Changes
1. **Size:** `32.dp → 64.dp`
2. **Color:** Yellow → Green (`Color(0xFF2E7D32)` - Material Green 800)

### Rationale
- Larger marker for better visibility on tablet display
- Green matches existing "live mode" color scheme (see MapView.kt line 583)

### Files
- `VehicleHeadingMarker.kt`:
  - Line 23: `vehicleHeadingMarkerSize = 64.dp`
  - Arrow brush color: Change to green

## 3. Stop Reference Disc

### Current State (MapView.kt lines 520-534)
```kotlin
drawCircle(
    color = Color.Red.copy(alpha = 0.6f),
    radius = refRadiusPx,
    center = Offset(px, py),
    style = Stroke(width = 2f)  // Outline circle
)
```

### Changes
- Remove `style = Stroke(width = 2f)` parameter
- Change `alpha = 0.6f` → `alpha = 0.3f` (softer fill)
- Result: Semi-transparent filled red disc

### Rationale
- "Disc" means filled, not outline
- Reduced alpha prevents obscuring route path underneath

### Files
- `MapView.kt` lines 529-534: Modify 50m reference circle draw call

## Implementation Order

1. VehicleHeadingMarker.kt (size + color)
2. MapView.kt (stop disc fill)
3. MapView.kt (camera follow toggle)

## Testing Checklist

- [ ] Camera follow toggle state persists across screen rotations
- [ ] Toggle icon changes (weak/strong) based on state
- [ ] Vehicle heading arrow appears 2x larger
- [ ] Arrow color is green (not yellow)
- [ ] 50m stop reference is filled (not outlined)
- [ ] Stop disc alpha ~0.3 (semi-transparent)
- [ ] No regression: map pan/zoom still works
- [ ] No regression: tiles load correctly
