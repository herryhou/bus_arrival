package com.busarrival.app.presentation.ui.detection.components

/**
 * Map view component with tile overlay and route rendering.
 *
 * Coordinate System:
 * - All world coordinates use baseZ=15 as stable reference zoom level
 * - offset, tileWorldX/Y, toScreenX/Y never change zoom
 * - Tiles at zoom Z scale by 2^(baseZ - Z) when drawn in baseZ space
 * - Fallback tiles compose scaling: 2^(baseZ - Z) * 2^(Z - F) = 2^(baseZ - F)
 *
 * This ensures geographic → screen conversion is stable across tileZ changes, preventing pan jumps
 * when zooming past tile boundaries.
 */
// Coordinate functions from MapCoordinateUtils.kt (same package)
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.busarrival.app.BuildConfig
import com.busarrival.app.data.cache.TileCache
import com.busarrival.app.domain.model.ReplayState
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.pow
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

data class LatLon(val lat: Double, val lon: Double)

fun RouteData.cmToLatLon(xCm: Int, yCm: Int): LatLon {
    // Match Rust code exactly: spherical Earth R = 6371 km
    val R_CM = 637100000.0
    val originLat = 20.0
    val originLon = 120.0
    val latAvg = avgLat.toDouble() / 1e6

    val latAvgRad = latAvg * PI / 180.0
    val cosLat = cos(latAvgRad)

    // Exactly matching Rust: x_abs = R * lon * cos(lat_avg)
    val x0Abs = originLon * PI / 180.0 * R_CM * cosLat
    val y0Abs = originLat * PI / 180.0 * R_CM

    val xAbs = x0Abs + xCm
    val yAbs = y0Abs + yCm

    val lat = yAbs / R_CM * 180.0 / PI
    val lon = xAbs / (R_CM * cosLat) * 180.0 / PI

    return LatLon(lat, lon)
}

@Composable
fun MapView(
        routeData: RouteData?,
        currentSCm: Int,
        isCameraFollowEnabled: Boolean,
        replayState: ReplayState = ReplayState(),
        viewModel: DetectionViewModel,
        modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    val tileDiskCache = remember { TileCache(context) }

    val scale by viewModel.mapScale.collectAsState()
    val offset by viewModel.mapOffset.collectAsState()
    val tileCache by viewModel.tileCache.collectAsState()

    // Log state on recomposition
    android.util.Log.d(
            "MapView",
            "State: scale=$scale, offset=$offset, cacheSize=${tileCache.size}"
    )
    val centerLatLon =
            remember(routeData) {
                routeData?.let {
                    val bounds = it.calculateBoundingBox()
                    if (bounds != null) {
                        it.cmToLatLon(
                                (bounds.minX + bounds.maxX) / 2,
                                (bounds.minY + bounds.maxY) / 2
                        )
                    } else null
                }
            }
    // Dynamically load higher zoom tiles when zoom level changes (not on every scale change)
    val baseZ = 15
    val tileZ =
            remember(scale) {
                (baseZ + (kotlin.math.ln(scale.toDouble()) / kotlin.math.ln(2.0)).toInt()).coerceIn(
                        12,
                        18
                )
            }

    // Single LaunchedEffect handles both initial preload and subsequent zoom/route changes
    // Keys on tileZ, centerLatLon, and cache state to avoid duplicate loading
    LaunchedEffect(tileZ, centerLatLon, tileCache.size) {
        centerLatLon ?: return@LaunchedEffect

        android.util.Log.d(
                "MapView",
                "Loading tiles: z=$tileZ (scale=$scale), center=$centerLatLon, cacheSize=${tileCache.size}"
        )

        // Load tiles for the current zoom level and route center
        val tiles =
                kotlin
                        .runCatching { loadTilesForZoom(centerLatLon, tileZ, tileDiskCache) }
                        .getOrNull()

        if (!tiles.isNullOrEmpty()) {
            viewModel.addTiles(tiles)
            android.util.Log.d("MapView", "Loaded ${tiles.size} tiles for z=$tileZ")
        }
    }

    Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        if (routeData != null && centerLatLon != null) {
            Canvas(
                    modifier =
                            Modifier.fillMaxSize().background(Color.White).pointerInput(Unit) {
                                detectTransformGestures { centroid, pan, zoom, _ ->
                                    val oldScale = scale
                                    val newScale = (oldScale * zoom).coerceIn(0.1f, 10f)

                                    // Convert centroid from screen coordinates to centered
                                    // coordinates
                                    // Screen origin is top-left, our offset origin is center
                                    val centroidCentered =
                                            centroid - Offset(size.width / 2f, size.height / 2f)

                                    // Adjust offset to keep pinch point stable: zoom around
                                    // centroid
                                    // Formula: offset += (centroid - offset) * (1 -
                                    // newScale/oldScale)
                                    val oldOffset = offset
                                    val scaleChange = 1 - newScale / oldScale
                                    val newOffset =
                                            oldOffset +
                                                    (centroidCentered - oldOffset) * scaleChange +
                                                    pan

                                    viewModel.updateMapState(newScale, newOffset)
                                }
                            }
            ) {
                val canvasWidth = size.width
                val canvasHeight = size.height

                val center = centerLatLon

                // Always use baseZ for positioning to prevent jumps
                val centerPixelX = lonToPixelX(center.lon, baseZ)
                val centerPixelY = latToPixelY(center.lat, baseZ)

                val centerTileX = lonToTileX(center.lon, tileZ)
                val centerTileY = latToTileY(center.lat, tileZ)
                val tileSize = 256f

                // Helper function to transform coordinates to screen space (must be defined before
                // tile drawing)
                // Uses baseZ for consistent coordinates across zoom levels (prevents jumping at
                // scale boundaries)
                /**
                 * Convert longitude to screen X coordinate.
                 *
                 * Uses baseZ for stable world coordinates across zoom changes. Geographic → screen
                 * conversion is invariant to tileZ changes.
                 *
                 * @param lon Geographic longitude
                 * @return Screen X coordinate in pixels
                 */
                fun toScreenX(lon: Double): Float {
                    val worldX = lonToPixelX(lon, baseZ) - lonToPixelX(center.lon, baseZ)
                    val result = worldX * scale + offset.x + canvasWidth / 2
                    if (BuildConfig.DEBUG) {
                        assert(result.isFinite()) { "Non-finite screen X coordinate: $result" }
                    }
                    return result
                }

                /**
                 * Convert latitude to screen Y coordinate.
                 *
                 * Uses baseZ for stable world coordinates across zoom changes. Geographic → screen
                 * conversion is invariant to tileZ changes.
                 *
                 * @param lat Geographic latitude
                 * @return Screen Y coordinate in pixels
                 */
                fun toScreenY(lat: Double): Float {
                    val worldY = latToPixelY(lat, baseZ) - latToPixelY(center.lat, baseZ)
                    val result = worldY * scale + offset.y + canvasHeight / 2
                    if (BuildConfig.DEBUG) {
                        assert(result.isFinite()) { "Non-finite screen Y coordinate: $result" }
                    }
                    return result
                }

                // Draw tiles with single outer transform (position in world space, let transform
                // handle scale/offset)
                withTransform({
                    translate(left = offset.x + canvasWidth / 2, top = offset.y + canvasHeight / 2)
                    scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
                }) {
                    // Position tiles in centered world space using tileZ for correct grid alignment
                    // Each zoom level has its own tile grid, so we must use tileZ for positioning
                    // Load more tiles at higher zoom levels
                    val tileRange =
                            when (tileZ) {
                                in 12..13 -> 3
                                in 14..15 -> 3
                                in 16..17 -> 4
                                else -> 5
                            }

                    android.util.Log.d(
                            "MapView",
                            "TILE RANGE: tileZ=$tileZ, tileRange=$tileRange, centerTileX=$centerTileX, centerTileY=$centerTileY, canvasW=$canvasWidth, canvasH=$canvasHeight"
                    )

                    var tilesDrawn = 0
                    var tilesMissing = 0
                    var minTileX = Int.MAX_VALUE
                    var maxTileX = Int.MIN_VALUE
                    var minTileY = Int.MAX_VALUE
                    var maxTileY = Int.MIN_VALUE

                    for (dx in -tileRange..tileRange) {
                        for (dy in -tileRange..tileRange) {
                            val tileX = centerTileX + dx
                            val tileY = centerTileY + dy

                            // Track tile boundaries
                            if (tileX < minTileX) minTileX = tileX
                            if (tileX > maxTileX) maxTileX = tileX
                            if (tileY < minTileY) minTileY = tileY
                            if (tileY > maxTileY) maxTileY = tileY

                            val key = "$tileZ/$tileX/$tileY"

                            // Try to get tile at current zoom level
                            var bitmap = tileCache[key]
                            var actualTileZ = tileZ
                            var actualTileX = tileX
                            var actualTileY = tileY

                            // Fallback: try lower zoom levels if current not available
                            if (bitmap == null && tileZ > 14) {
                                var foundFallback = false
                                for (fallbackZ in (tileZ - 1) downTo 14) {
                                    if (foundFallback) break
                                    val fallbackX = tileX / 2.0.pow(tileZ - fallbackZ).toInt()
                                    val fallbackY = tileY / 2.0.pow(tileZ - fallbackZ).toInt()
                                    val fallbackKey = "$fallbackZ/$fallbackX/$fallbackY"
                                    tileCache[fallbackKey]?.let {
                                        bitmap = it
                                        actualTileZ = fallbackZ
                                        actualTileX = fallbackX
                                        actualTileY = fallbackY
                                        android.util.Log.d(
                                                "MapView",
                                                "Using fallback z=$fallbackZ for $key"
                                        )
                                        foundFallback = true
                                    }
                                }
                            }

                            if (bitmap == null) {
                                tilesMissing++
                                if (tilesMissing <= 5) { // Log first 5 missing
                                    android.util.Log.d("MapView", "MISSING: $key")
                                }
                            }

                            bitmap?.let {
                                // Calculate position using REQUESTED tile coordinates (tileX, tileY
                                // at tileZ)
                                // Convert geographic to baseZ world space directly (not via
                                // worldX/Y helpers)
                                val tileNW = tileXToLon(tileX, tileZ)
                                val tileNE = tileYToLat(tileY, tileZ)

                                // Position in baseZ world space for stable coordinates across zoom
                                // MUST use lonToPixelX/latToPixelY with baseZ directly, NOT
                                // worldX/Y helpers
                                val tileWorldX =
                                        lonToPixelX(tileNW, baseZ) - lonToPixelX(center.lon, baseZ)
                                val tileWorldY =
                                        latToPixelY(tileNE, baseZ) - latToPixelY(center.lat, baseZ)

                                if (BuildConfig.DEBUG) {
                                    assert(tileWorldX.isFinite() && tileWorldY.isFinite()) {
                                        "Non-finite world coordinates: x=$tileWorldX, y=$tileWorldY"
                                    }
                                }

                                tilesDrawn++
                                if (tilesDrawn <= 5) { // Log first 5 drawn with full details
                                    // Calculate screen position for debugging
                                    val screenX = tileWorldX * scale + offset.x + canvasWidth / 2
                                    val screenY = tileWorldY * scale + offset.y + canvasHeight / 2
                                    val screenSize = tileSize * scale
                                    val screenEndX = screenX + screenSize
                                    val screenEndY = screenY + screenSize

                                    android.util.Log.d(
                                            "MapView",
                                            "DRAW: $key -> actualZ=$actualTileZ, tileZ=$tileZ"
                                    )
                                    android.util.Log.d(
                                            "MapView",
                                            "  worldX=$tileWorldX, worldY=$tileWorldY"
                                    )
                                    android.util.Log.d(
                                            "MapView",
                                            "  screen: x=$screenX, y=$screenY, size=$screenSize"
                                    )
                                    android.util.Log.d(
                                            "MapView",
                                            "  covers: x=[$screenX,$screenEndX], y=[$screenY,$screenEndY]"
                                    )
                                }

                                // Scale tile to match zoom level difference between tileZ and baseZ
                                val zoomScaleFactor = 2.0.pow(baseZ - tileZ).toFloat()
                                // If using fallback tile, also scale by difference between
                                // requested and actual
                                val fallbackScaleFactor =
                                        if (actualTileZ != tileZ) {
                                            2.0.pow(tileZ - actualTileZ).toFloat()
                                        } else {
                                            1f
                                        }
                                val totalScale = zoomScaleFactor * fallbackScaleFactor

                                if (BuildConfig.DEBUG) {
                                    assert(totalScale > 0f && totalScale < 1000f) {
                                        "Invalid scale factor: $totalScale (tileZ=$tileZ, baseZ=$baseZ, actualTileZ=$actualTileZ)"
                                    }
                                }

                                if (totalScale != 1f) {
                                    android.util.Log.d(
                                            "MapView",
                                            "  SCALING: total=$totalScale (zoom=$zoomScaleFactor, fallback=$fallbackScaleFactor)"
                                    )
                                    withTransform({
                                        scale(
                                                scaleX = totalScale,
                                                scaleY = totalScale,
                                                pivot = Offset(tileWorldX, tileWorldY)
                                        )
                                    }) {
                                        drawImage(
                                                image = it,
                                                topLeft = Offset(tileWorldX, tileWorldY),
                                                alpha = if (actualTileZ != tileZ) 0.7f else 1f
                                        )
                                    }
                                } else {
                                    drawImage(image = it, topLeft = Offset(tileWorldX, tileWorldY))
                                }
                            }
                        }
                    }

                    android.util.Log.d(
                            "MapView",
                            "SUMMARY: tileRange=$tileRange, totalRequested=${(tileRange*2+1)*(tileRange*2+1)}, tilesDrawn=$tilesDrawn, tilesMissing=$tilesMissing"
                    )
                    android.util.Log.d(
                            "MapView",
                            "BOUNDARIES: minTileX=$minTileX, maxTileX=$maxTileX, minTileY=$minTileY, maxTileY=$maxTileY"
                    )
                    android.util.Log.d(
                            "MapView",
                            "COORDINATES: minTileNW=${tileXToLon(minTileX, tileZ)}, maxTileNE=${tileYToLat(maxTileY, tileZ)}"
                    )
                } // End outer withTransform for tiles

                // Draw route and markers with scale-appropriate stroke width
                // Stroke width scales inversely with zoom to maintain consistent visual weight
                val path = Path()
                routeData.nodes.forEachIndexed { index, node ->
                    val ll = routeData.cmToLatLon(node.xCm, node.yCm)
                    val px = toScreenX(ll.lon)
                    val py = toScreenY(ll.lat)

                    if (index == 0) {
                        path.moveTo(px, py)
                    } else {
                        path.lineTo(px, py)
                    }
                }

                // Constant stroke width (does not scale with zoom)
                val strokeWidth = 8f
                drawPath(
                        path = path,
                        color = Color.Blue,
                        style = Stroke(width = strokeWidth, pathEffect = null)
                )

                // Draw stops with constant radius (interpolated along segments)
                val stopRadius = 8f
                routeData.stops.forEach { stop ->
                    val pos = routeData.interpolatePosition(stop.progressCm)
                    if (pos != null) {
                        val ll = routeData.cmToLatLon(pos.first, pos.second)
                        val px = toScreenX(ll.lon)
                        val py = toScreenY(ll.lat)
                        drawCircle(color = Color.Red, radius = stopRadius, center = Offset(px, py))
                    }
                }

                // Draw current position (live or replay) with scaled radius (interpolated)
                // In replay mode, currentSCm is already updated by ViewModel's
                // updateUiForPosition()
                val pos = routeData.interpolatePosition(currentSCm)
                if (pos != null) {
                    val ll = routeData.cmToLatLon(pos.first, pos.second)
                    val px = toScreenX(ll.lon)
                    val py = toScreenY(ll.lat)

                    // Use different colors for live vs replay mode
                    val markerColor =
                            if (replayState.traceFile != null) {
                                Color.Blue // Replay mode: blue marker
                            } else {
                                Color.Green // Live mode: green marker
                            }

                    val markerRadius = 12f
                    drawCircle(color = markerColor, radius = markerRadius, center = Offset(px, py))
                }
            }

            Column(modifier = Modifier.align(Alignment.TopStart).padding(16.dp)) {
                Text(
                        text = "Nodes: ${routeData.nodes.size}",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Black
                )
                Text(
                        text = "Stops: ${routeData.stops.size}",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Black
                )
                val modeLabel = if (replayState.traceFile != null) "REPLAY" else "LIVE"
                Text(
                        text =
                                "$modeLabel | Progress: ${currentSCm / 100}m | Zoom: ${"%.2f".format(scale)}x",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Black
                )
                Text(
                        text = "Tiles: ${tileCache.size}",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Black
                )
                // Debug: test origin conversion
                val testOrigin = routeData.cmToLatLon(0, 0)
                Text(
                        text =
                                "Center: ${"%.4f".format(centerLatLon!!.lat)}, ${"%.4f".format(centerLatLon!!.lon)}",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Black
                )
                Text(
                        text =
                                "Origin(0,0): ${"%.4f".format(testOrigin.lat)}, ${"%.4f".format(testOrigin.lon)}",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Black
                )

                // Debug button to clear tile cache
                androidx.compose.material3.Button(
                        onClick = { viewModel.clearTileCache() },
                        modifier = Modifier.padding(top = 8.dp),
                        colors =
                                androidx.compose.material3.ButtonDefaults.buttonColors(
                                        containerColor = Color.Red
                                )
                ) {
                    androidx.compose.material3.Text(
                            "Clear Tiles",
                            style = MaterialTheme.typography.bodySmall
                    )
                }
            }
        } else {
            Text(
                    text = "No route loaded",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.error
            )
        }
    }
}

private fun RouteData.calculateBoundingBox(): BoundingBoxData? {
    if (nodes.isEmpty()) return null

    var minX = Int.MAX_VALUE
    var minY = Int.MAX_VALUE
    var maxX = Int.MIN_VALUE
    var maxY = Int.MIN_VALUE

    nodes.forEach { node ->
        minX = minOf(minX, node.xCm)
        minY = minOf(minY, node.yCm)
        maxX = maxOf(maxX, node.xCm)
        maxY = maxOf(maxY, node.yCm)
    }

    return BoundingBoxData(minX, minY, maxX, maxY)
}

private fun RouteData.findNodeAtProgress(progressCm: Int): RouteNode? {
    var lastNode = nodes.firstOrNull()
    for (node in nodes) {
        if (node.cumDistCm >= progressCm) return lastNode
        lastNode = node
    }
    return lastNode
}

/**
 * Interpolate position along route at given progress. Returns interpolated (x, y) coordinates in
 * centimeters.
 */
private fun RouteData.interpolatePosition(progressCm: Int): Pair<Int, Int>? {
    if (nodes.isEmpty()) return null

    // Find the segment we're on
    var prevNode: RouteNode? = null
    for (node in nodes) {
        if (node.cumDistCm >= progressCm) {
            // Found the node we've passed or reached
            prevNode?.let { prev ->
                // We're in the middle of a segment, interpolate
                val segmentProgressCm = progressCm - prev.cumDistCm
                val segmentLenCm = prev.segLenMm / 10 // Convert mm to cm

                if (segmentLenCm > 0) {
                    // Interpolate along the segment
                    val ratio = segmentProgressCm.toFloat() / segmentLenCm.toFloat()
                    val x = prev.xCm + (prev.dxCm.toFloat() * ratio).toInt()
                    val y = prev.yCm + (prev.dyCm.toFloat() * ratio).toInt()
                    return Pair(x, y)
                }
            }
            // At or before first node, return node position
            return Pair(node.xCm, node.yCm)
        }
        prevNode = node
    }

    // Past the last node, return last node position
    return prevNode?.let { Pair(it.xCm, it.yCm) }
}

private data class BoundingBoxData(val minX: Int, val minY: Int, val maxX: Int, val maxY: Int)

// Sync version for LaunchedEffect (run in coroutine)
// Uses disk cache for offline support
private suspend fun preloadTilesSync(
        center: LatLon,
        diskCache: TileCache
): Map<String, ImageBitmap> =
        withContext(Dispatchers.IO) {
            val cache = mutableMapOf<String, ImageBitmap>()

            for (z in 14..16) {
                val tileX = lonToTileX(center.lon, z)
                val tileY = latToTileY(center.lat, z)
                for (dx in -2..2) {
                    for (dy in -2..2) {
                        val x = tileX + dx
                        val y = tileY + dy
                        val key = "$z/$x/$y"

                        if (!cache.containsKey(key)) {
                            // Check disk cache first
                            var bitmap = diskCache.getTile(z, x, y)

                            // Fetch from network if not in disk cache
                            if (bitmap == null) {
                                bitmap = diskCache.fetchAndCacheTile(z, x, y)
                            }

                            // Convert to ImageBitmap for memory cache
                            bitmap?.let { cache[key] = it.asImageBitmap() }
                        }
                    }
                }
            }

            android.util.Log.d(
                    "MapView",
                    "Disk cache: ${diskCache.getCachedTileCount()} tiles, ${diskCache.getCacheSize() / 1024}KB"
            )
            cache
        }

/** Load tiles for a specific zoom level (for dynamic loading when zooming). */
private suspend fun loadTilesForZoom(
        center: LatLon,
        zoom: Int,
        diskCache: TileCache
): Map<String, ImageBitmap> =
        withContext(Dispatchers.IO) {
            val cache = mutableMapOf<String, ImageBitmap>()
            val range =
                    when (zoom) {
                        in 0..14 -> 3
                        in 15..18 -> 4
                        else -> 4
                    }

            val tileX = lonToTileX(center.lon, zoom)
            val tileY = latToTileY(center.lat, zoom)

            for (dx in -range..range) {
                for (dy in -range..range) {
                    val x = tileX + dx
                    val y = tileY + dy
                    val key = "$zoom/$x/$y"

                    if (!cache.containsKey(key)) {
                        // Check disk cache first
                        var bitmap = diskCache.getTile(zoom, x, y)

                        // Fetch from network if not in disk cache
                        if (bitmap == null) {
                            bitmap = diskCache.fetchAndCacheTile(zoom, x, y)
                        }

                        // Convert to ImageBitmap for memory cache
                        bitmap?.let { cache[key] = it.asImageBitmap() }
                    }
                }
            }

            cache
        }
