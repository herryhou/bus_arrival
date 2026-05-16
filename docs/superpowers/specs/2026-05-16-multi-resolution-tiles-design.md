# Multi-Resolution Map Tiles Design

**Date:** 2026-05-16
**Status:** Approved
**Author:** Claude (User-Specified)

## Problem Statement

Current Android map tile system hardcodes `@2x` (512x512px) tiles for all zoom levels. This wastes bandwidth at low zoom (Z14-15) where 256x256 tiles suffice, and limits sharpness at high zoom (Z19-20) where larger tiles would improve clarity.

## Requirements

1. **Performance optimization** — download smaller tiles at low zoom, larger tiles at high zoom
2. **Aggressive resolution mapping** — 1x at Z14-15, 2x at Z16-17, 4x at Z18, 8x at Z19-20
3. **Hybrid fallback** — immediate fallback for display, background download for replacement
4. **Separate cache files** — each resolution cached independently

## Resolution Mapping

| Zoom Range | Resolution | URL Suffix | Tile Size |
|------------|------------|------------|-----------|
| Z14-15     | 1x         | ``         | 256×256   |
| Z16-17     | 2x         | `@2x`      | 512×512   |
| Z18        | 4x         | `@4x`      | 1024×1024 |
| Z19-20     | 8x         | `@8x`      | 2048×2048 |

## Architecture

### 1. Resolution Enum

```kotlin
enum class TileResolution(val scale: Int, val urlSuffix: String) {
    X1(1, ""),
    X2(2, "@2x"),
    X4(4, "@4x"),
    X8(8, "@8x");

    companion object {
        fun forZoom(zoom: Int): TileResolution = when (zoom) {
            in 14..15 -> X1
            in 16..17 -> X2
            18       -> X4
            in 19..20 -> X8
            else     -> X2  // fallback
        }
    }
}
```

### 2. TileCache API Changes

**Before:**
```kotlin
suspend fun getTile(z: Int, x: Int, y: Int): Bitmap?
suspend fun fetchAndCacheTile(z: Int, x: Int, y: Int): Bitmap?
private fun getTileFile(z: Int, x: Int, y: Int): File
```

**After:**
```kotlin
suspend fun getTile(z: Int, x: Int, y: Int, resolution: TileResolution): Bitmap?
suspend fun fetchAndCacheTile(z: Int, x: Int, y: Int, resolution: TileResolution): Bitmap?
private fun getTileFile(z: Int, x: Int, y: Int, resolution: TileResolution): File
```

**Cache file naming:** `cacheDir/{z}/{x}/{y}@{scale}.png`

Examples:
- `cacheDir/14/123/456@1.png`
- `cacheDir/17/123/456@2.png`
- `cacheDir/20/123/456@8.png`

### 3. MapView Changes

**Tile cache key format:** `"{z}/{x}/{y}@{scale}"`

**Load function signature:**
```kotlin
private suspend fun loadTilesForZoom(
    center: LatLon,
    zoom: Int,
    diskCache: TileCache,
    tileRange: Int,
    resolution: TileResolution
): Map<String, ImageBitmap>
```

**Resolution-aware tile loading:**
```kotlin
val resolution = TileResolution.forZoom(tileZ)
val tiles = loadTilesForZoom(centerLatLon, tileZ, tileDiskCache, fetchRange, resolution)
```

### 4. Hybrid Fallback Logic

```kotlin
suspend fun loadTileWithFallback(
    z: Int, x: Int, y: Int,
    diskCache: TileCache,
    targetRes: TileResolution
): Bitmap? {
    // Try exact resolution first
    var bitmap = diskCache.fetchAndCacheTile(z, x, y, targetRes)
    if (bitmap != null) return bitmap

    // Fallback to lower resolutions for immediate display
    val fallbackOrder = listOf(
        TileResolution.X4, TileResolution.X2, TileResolution.X1
    ).filter { it.scale < targetRes.scale }

    for (fallbackRes in fallbackOrder) {
        bitmap = diskCache.getTile(z, x, y, fallbackRes)
        if (bitmap != null) {
            // Trigger background fetch of target resolution
            CoroutineScope(Dispatchers.IO).launch {
                diskCache.fetchAndCacheTile(z, x, y, targetRes)
            }
            return bitmap
        }
    }

    return null
}
```

### 5. Draw Scale Adjustment

Current code divides by 2 for `@2x` tiles:
```kotlin
val drawScale = 2.0f.pow(baseZ - posZ) / 2f
```

**Updated for variable resolution:**
```kotlin
val resolutionScale = when (actualTileZ) {
    in 14..15 -> 1f
    in 16..17 -> 2f
    18       -> 4f
    in 19..20 -> 8f
    else     -> 2f
}
val drawScale = 2.0f.pow(baseZ - posZ) / resolutionScale
```

Or use cache key parsing:
```kotlin
val resolutionScale = extractScaleFromCacheKey(key).toFloat()
val drawScale = 2.0f.pow(baseZ - posZ) / resolutionScale
```

## Data Flow

```
User zooms to Z18
    ↓
MapView.tileZ = 18
    ↓
resolution = TileResolution.forZoom(18) = X4
    ↓
loadTilesForZoom(..., resolution=X4)
    ↓
TileCache.fetchAndCacheTile(z, x, y, X4)
    ↓
URL: "https://a.basemaps.cartocdn.com/light_all/18/x/y@4x.png"
    ↓
Cache: "cacheDir/18/x/y@4.png"
    ↓
If missing: fallback to X2, background fetch X4
    ↓
Draw with drawScale = 2^(baseZ-posZ) / 4
```

## Testing

### Unit Tests

1. `resolutionForZoom()` returns correct values for Z14-20
2. Cache file path generation for all resolutions
3. Fallback chain ordering

### Integration Tests

1. Fetch and cache 1x, 2x, 4x, 8x tiles
2. Verify disk cache structure
3. Background fetch triggered on fallback

### UI Tests

1. Visual quality at Z14 (1x), Z17 (2x), Z18 (4x), Z20 (8x)
2. Fallback rendering smoothness
3. No gaps between tiles at resolution boundaries

## Implementation Notes

1. **CartoDB tile availability:** Verify `@4x` and `@8x` tiles are served by CartoDB Light All
2. **Memory usage:** 8x tiles are 2048×2048 = 16MB uncompressed — limit concurrent in-memory cache
3. **Network bandwidth:** 8x tiles are ~4x larger than 2x — consider prefetch throttling
4. **Existing bugs:** Fix fallback positioning bug (see claude_review.md issue #1) before implementing

## Success Criteria

1. Z14-15 uses 1x tiles (verified via network logs)
2. Z19-20 uses 8x tiles (visual sharpness improved)
3. Fallback tiles display immediately, high-res replaces async
4. No increase in memory crashes
5. Cache eviction works correctly across resolutions
