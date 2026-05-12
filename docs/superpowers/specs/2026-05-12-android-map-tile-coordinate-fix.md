# Android Map Tile Coordinate Fix

## Problem

Three valid reviews identified coordinate system bugs in `MapView.kt`:

1. **Scaling Direction (High)**: Tiles at `tileZ > baseZ` are scaled up instead of down. Formula `2^(tileZ - baseZ)` produces wrong size.
2. **Offset Rescaling (High)**: SideEffect at lines 100-116 adjusts offset on `tileZ` changes, but no coordinate system change occurs (everything already in baseZ). Causes spurious pan jumps.
3. **Test Failures (Medium)**: Four `FallbackTileTest` assertions fail due to issues 1 and 2.

## Root Cause

Missing invariants. The code migrated to `baseZ` world coordinates but left behind:
- Incorrect scaling exponent (from pre-baseZ logic)
- Unnecessary offset adjustment (from pre-baseZ logic)
- Inconsistent test helpers (mixed coordinate systems)

## Design

### Phase 1: Immediate Bug Fixes

**Fix 1 - Scaling Direction**
- Location: `MapView.kt:283`
- Change: `2.0.pow(tileZ - baseZ)` → `2.0.pow(baseZ - tileZ)`
- Effect: High-zoom tiles shrink correctly in baseZ world space
- Example: tileZ=17, baseZ=15 → scale = 1/4 (not 4)

**Fix 2 - Remove Offset Rescaling**
- Location: `MapView.kt:100-116`
- Action: Delete entire `prevTileZ` SideEffect block
- Rationale: No coordinate system change occurs at tileZ boundaries
- Effect: Eliminates spurious pan jumps during zoom

**Fix 3 - Test Consistency**
- Location: `FallbackTileTest.kt:28-51`
- Change: `calculateTileScreenPosition()` to use `baseZ` parameter
- Match: Production code's coordinate system
- Effect: 4 failing assertions pass

### Phase 2: Coordinate Invariants

**Invariant 1 - BaseZ Stability**
- All world coordinates use `baseZ=15` as reference zoom level
- `offset`, `tileWorldX/Y`, `toScreenX/Y` never change zoom
- Geographic → screen conversion is stable across tileZ changes

**Invariant 2 - Tile Scaling**
- Tile at zoom Z displayed in baseZ coordinates: scale = `2^(baseZ - Z)`
- Higher Z tiles appear smaller; lower Z tiles appear larger
- Examples:
  - Z=17 tile at baseZ=15: scale = 1/4 (256px → 64px)
  - Z=14 tile at baseZ=15: scale = 2 (256px → 512px)

**Invariant 3 - Fallback Positioning**
- Fallback from zoom F used for request Z: position using Z, scale by `2^(Z - F)`
- Ensures fallback tiles cover identical geographic area as native
- Example: Z=17 request, F=15 fallback → position at Z=17 grid, scale 4x

**Documentation Additions**
- Kdoc on `toScreenX/Y` explaining baseZ contract
- Inline comment at line 263 explaining world coordinate choice
- File header with coordinate system overview

### Phase 3: Debug Assertions

**Assertion 1 - Scale Factor Sanity**
```kotlin
assert(totalScale > 0f && totalScale < 1000f) {
    "Invalid scale factor: $totalScale (tileZ=$tileZ, baseZ=$baseZ, actualTileZ=$actualTileZ)"
}
```

**Assertion 2 - World Coordinate Bounds**
```kotlin
assert(tileWorldX.isFinite() && tileWorldY.isFinite()) {
    "Non-finite world coordinates: x=$tileWorldX, y=$tileWorldY"
}
```

**Assertion 3 - Screen Coordinate Bounds**
```kotlin
val result = worldX * scale + offset.x + canvasWidth / 2
assert(result.isFinite()) { "Non-finite screen coordinate: $result" }
```

All assertions wrapped in `if (BuildConfig.DEBUG)` for zero release cost.

## Success Criteria

1. All 4 `FallbackTileTest` assertions pass
2. No pan jumps when crossing tileZ boundaries during zoom
3. Visual tile alignment correct at all zoom levels
3. No new regressions in gesture handling

## Files Modified

- `android/app/src/main/java/com/busarrival/app/presentation/ui/detection/components/MapView.kt`
- `android/app/src/test/java/com/busarrival/app/presentation/ui/detection/components/FallbackTileTest.kt`
