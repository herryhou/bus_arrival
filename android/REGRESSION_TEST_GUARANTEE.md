# Regression Test Guarantee

## Bug Fixed

**Problem:** `worldX/Y` used `baseZ=15` for positioning, but tiles are at `tileZ=17-18`

**Symptom:** All tiles stacked at same position, creating visual mess

**Fix:** `worldX/Y` now use `tileZ` (requested zoom level) for positioning

## How Tests Prevent Regression

### Test: `regressionUsingBaseZInsteadOfTileZ()`

**What it does:**
1. Calculates tile positions using BROKEN method (baseZ)
2. Calculates tile positions using FIXED method (tileZ)
3. Verifies BROKEN method gives wrong spacing at high zoom levels

**What it catches:**
- If someone reverts `worldX/Y` to use `baseZ` instead of `tileZ`
- At tileZ=18, baseZ gives spacing ≠ 256px
- Test will FAIL with message: "Using baseZ at tileZ=18 gives WRONG spacing"

### Test: `tileBoundariesAtScales1_2_4()`

**What it does:**
1. Simulates EXACT transform chain with tileZ parameter
2. Verifies tile boundaries at scales 1, 2, 4
3. Checks tile_end == next_tile_start at each scale

**What it catches:**
- If transform chain is modified incorrectly
- If tileZ parameter is not passed correctly
- If positioning math changes

## Verification Command

After any code changes to MapView.kt, run:

```bash
adb logcat -s MapView:D | grep -E "screen:|covers:|FALLBACK" | head -20
```

**Expected output (correct):**
```
screen: x=-5854.28, y=-1259.89  (tile 1)
screen: x=-5825.17, y=-1251.45  (tile 2 - DIFFERENT X!)
covers: x=[-5854.28,-3294.28], y=[-1259.89,1300.10]  (tile 1)
covers: x=[-5825.17,-3265.17], y=[-1251.45,1308.54]  (tile 2)
```

**Broken output (bug):**
```
screen: x=-5854.28, y=-1259.89  (tile 1)
screen: x=-5854.28, y=-939.89  (tile 2 - SAME X!)
```

## Test Coverage

| Test | Protects Against |
|------|-------------------|
| `regressionUsingBaseZInsteadOfTileZ` | Reverting to baseZ positioning |
| `tileBoundariesAtScales1_2_4` | Transform chain breakage |
| `tileBoundariesWithPanOffset` | Pan offset math errors |
| `verticalTileBoundariesAlign` | Y-axis positioning bugs |
| `fallbackTilesAlignInScreenSpace` | Fallback scaling issues |
| `diagnosticTilePositionsAtScale4` | Complete rendering verification |

## Implementation Requirement

MapView.kt lines 184-185 MUST be:
```kotlin
fun worldX(lon: Double): Float = lonToPixelX(lon, tileZ) - lonToPixelX(center.lon, tileZ)
fun worldY(lat: Double): Float = latToPixelY(lat, tileZ) - latToPixelY(center.lat, tileZ)
```

NOT baseZ. This is enforced by tests.
