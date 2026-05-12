package com.busarrival.app.presentation.ui.detection.components

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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.busarrival.app.data.cache.TileCache
import com.busarrival.app.domain.model.ReplayState
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
// Coordinate functions from MapCoordinateUtils.kt (same package)
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.pow

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
    android.util.Log.d("MapView", "State: scale=$scale, offset=$offset, cacheSize=${tileCache.size}")
    val centerLatLon = remember {
        routeData?.let {
            val bounds = it.calculateBoundingBox()
            if (bounds != null) {
                it.cmToLatLon((bounds.minX + bounds.maxX) / 2, (bounds.minY + bounds.maxY) / 2)
            } else null
        }
    }

    // Only preload tiles if cache is empty
    LaunchedEffect(centerLatLon) {
        android.util.Log.d("MapView", "LaunchedEffect triggered, centerLatLon=$centerLatLon, cacheSize=${tileCache.size}")
        if (centerLatLon != null && tileCache.isEmpty()) {
            android.util.Log.d("MapView", "Preloading tiles for center=$centerLatLon")
            val tiles = preloadTilesSync(centerLatLon, tileDiskCache)
            android.util.Log.d("MapView", "Preloaded ${tiles.size} tiles")
            viewModel.addTiles(tiles)
        }
    }

    // Dynamically load higher zoom tiles when zoom level changes (not on every scale change)
    val baseZ = 15
    val tileZ = remember(scale) {
        (baseZ + (kotlin.math.ln(scale.toDouble()) / kotlin.math.ln(2.0)).toInt()).coerceIn(12, 18)
    }

    LaunchedEffect(tileZ) {
        centerLatLon ?: return@LaunchedEffect

        android.util.Log.d("MapView", "Zoom level changed to z=$tileZ (scale=$scale), loading tiles...")

        // Load tiles for the new zoom level
        val tiles = kotlin.runCatching {
            loadTilesForZoom(centerLatLon, tileZ, tileDiskCache)
        }.getOrNull()

        if (!tiles.isNullOrEmpty()) {
            viewModel.addTiles(tiles)
            android.util.Log.d("MapView", "Loaded ${tiles.size} tiles for z=$tileZ")
        }
    }

    Box(
        modifier = modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        if (routeData != null && centerLatLon != null) {
            Canvas(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.White)
                    .pointerInput(Unit) {
                        detectTransformGestures { centroid, pan, zoom, _ ->
                            val oldScale = scale
                            val newScale = (oldScale * zoom).coerceIn(0.1f, 10f)

                            val oldOffset = offset
                            val newOffset = oldOffset + (centroid - oldOffset) * (1 - newScale / oldScale) + pan

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

                // Helper function to transform coordinates to screen space (must be defined before tile drawing)
                // Uses tileZ to match tile coordinate system for proper alignment at all zoom levels
                // Transform MUST match tile transform: scale from origin, then translate by offset+center
                fun toScreenX(lon: Double): Float {
                    val worldX = lonToPixelX(lon, tileZ) - lonToPixelX(center.lon, tileZ)
                    return worldX * scale + offset.x + canvasWidth / 2
                }

                fun toScreenY(lat: Double): Float {
                    val worldY = latToPixelY(lat, tileZ) - latToPixelY(center.lat, tileZ)
                    return worldY * scale + offset.y + canvasHeight / 2
                }

                // Draw tiles with single outer transform (position in world space, let transform handle scale/offset)
                withTransform({
                    translate(left = offset.x + canvasWidth / 2, top = offset.y + canvasHeight / 2)
                    scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
                }) {
                    // Position tiles in centered world space using tileZ for correct grid alignment
                    // Each zoom level has its own tile grid, so we must use tileZ for positioning
                    // Load more tiles at higher zoom levels
                    val tileRange = when (tileZ) {
                        in 12..13 -> 3
                        in 14..15 -> 3
                        in 16..17 -> 4
                        else -> 5
                    }

                    android.util.Log.d("MapView", "TILE RANGE: tileZ=$tileZ, tileRange=$tileRange, centerTileX=$centerTileX, centerTileY=$centerTileY, canvasW=$canvasWidth, canvasH=$canvasHeight")

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
                                        android.util.Log.d("MapView", "Using fallback z=$fallbackZ for $key")
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
                                // Calculate position using REQUESTED tile coordinates (tileX, tileY at tileZ)
                                val tileNW = tileXToLon(tileX, tileZ)
                                val tileNE = tileYToLat(tileY, tileZ)

                                // Position in world space using tileZ (consistent for all tiles)
                                val tileWorldX = worldX(tileNW, center.lon, tileZ)
                                val tileWorldY = worldY(tileNE, center.lat, tileZ)

                                tilesDrawn++
                                if (tilesDrawn <= 5) { // Log first 5 drawn with full details
                                    // Calculate screen position for debugging
                                    val screenX = tileWorldX * scale + offset.x + canvasWidth / 2
                                    val screenY = tileWorldY * scale + offset.y + canvasHeight / 2
                                    val screenSize = tileSize * scale
                                    val screenEndX = screenX + screenSize
                                    val screenEndY = screenY + screenSize

                                    android.util.Log.d("MapView", "DRAW: $key -> actualZ=$actualTileZ, tileZ=$tileZ")
                                    android.util.Log.d("MapView", "  worldX=$tileWorldX, worldY=$tileWorldY")
                                    android.util.Log.d("MapView", "  screen: x=$screenX, y=$screenY, size=$screenSize")
                                    android.util.Log.d("MapView", "  covers: x=[$screenX,$screenEndX], y=[$screenY,$screenEndY]")
                                }

                                // If using fallback tile, scale it to match requested zoom level
                                if (actualTileZ != tileZ) {
                                    val zoomScaleFactor = 2.0.pow(tileZ - actualTileZ).toFloat()
                                    android.util.Log.d("MapView", "  FALLBACK: scaling by ${zoomScaleFactor}x")
                                    withTransform({
                                        scale(scaleX = zoomScaleFactor, scaleY = zoomScaleFactor, pivot = Offset(tileWorldX, tileWorldY))
                                    }) {
                                        drawImage(image = it, topLeft = Offset(tileWorldX, tileWorldY), alpha = 0.7f)
                                    }
                                } else {
                                    drawImage(image = it, topLeft = Offset(tileWorldX, tileWorldY))
                                }
                            }
                        }
                    }

                    android.util.Log.d("MapView", "SUMMARY: tileRange=$tileRange, totalRequested=${(tileRange*2+1)*(tileRange*2+1)}, tilesDrawn=$tilesDrawn, tilesMissing=$tilesMissing")
                    android.util.Log.d("MapView", "BOUNDARIES: minTileX=$minTileX, maxTileX=$maxTileX, minTileY=$minTileY, maxTileY=$maxTileY")
                    android.util.Log.d("MapView", "COORDINATES: minTileNW=${tileXToLon(minTileX, tileZ)}, maxTileNE=${tileYToLat(maxTileY, tileZ)}")
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

                // Scale stroke width by zoom level (thinner at high zoom, thicker at low zoom)
                val strokeWidth = 4f / scale.coerceAtLeast(0.5f)
                drawPath(
                    path = path,
                    color = Color.Blue,
                    style = Stroke(width = strokeWidth, pathEffect = null)
                )

                // Draw stops with scaled radius
                val stopRadius = 8f / scale.coerceAtLeast(0.5f)
                routeData.stops.forEach { stop ->
                    val node = routeData.findNodeAtProgress(stop.progressCm)
                    if (node != null) {
                        val ll = routeData.cmToLatLon(node.xCm, node.yCm)
                        val px = toScreenX(ll.lon)
                        val py = toScreenY(ll.lat)
                        drawCircle(
                            color = Color.Red,
                            radius = stopRadius,
                            center = Offset(px, py)
                        )
                    }
                }

                // Draw current position (live or replay) with scaled radius
                // In replay mode, currentSCm is already updated by ViewModel's updateUiForPosition()
                val currentNode = routeData.findNodeAtProgress(currentSCm)
                if (currentNode != null) {
                    val ll = routeData.cmToLatLon(currentNode.xCm, currentNode.yCm)
                    val px = toScreenX(ll.lon)
                    val py = toScreenY(ll.lat)

                    // Use different colors for live vs replay mode
                    val markerColor = if (replayState.traceFile != null) {
                        Color.Blue  // Replay mode: blue marker
                    } else {
                        Color.Green // Live mode: green marker
                    }

                    val markerRadius = 12f / scale.coerceAtLeast(0.5f)
                    drawCircle(
                        color = markerColor,
                        radius = markerRadius,
                        center = Offset(px, py)
                    )
                }
            }

            Column(
                modifier = Modifier
                    .align(Alignment.TopStart)
                    .padding(16.dp)
            ) {
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
                    text = "$modeLabel | Progress: ${currentSCm / 100}m | Zoom: ${"%.2f".format(scale)}x",
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
                    text = "Center: ${"%.4f".format(centerLatLon!!.lat)}, ${"%.4f".format(centerLatLon!!.lon)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = Color.Black
                )
                Text(
                    text = "Origin(0,0): ${"%.4f".format(testOrigin.lat)}, ${"%.4f".format(testOrigin.lon)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = Color.Black
                )

                // Debug button to clear tile cache
                androidx.compose.material3.Button(
                    onClick = { viewModel.clearTileCache() },
                    modifier = Modifier.padding(top = 8.dp),
                    colors = androidx.compose.material3.ButtonDefaults.buttonColors(
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

private data class BoundingBoxData(
    val minX: Int,
    val minY: Int,
    val maxX: Int,
    val maxY: Int
)

// Sync version for LaunchedEffect (run in coroutine)
// Uses disk cache for offline support
private suspend fun preloadTilesSync(center: LatLon, diskCache: TileCache): Map<String, ImageBitmap> = withContext(Dispatchers.IO) {
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
                    bitmap?.let {
                        cache[key] = it.asImageBitmap()
                    }
                }
            }
        }
    }

    android.util.Log.d("MapView", "Disk cache: ${diskCache.getCachedTileCount()} tiles, ${diskCache.getCacheSize() / 1024}KB")
    cache
}

/**
 * Load tiles for a specific zoom level (for dynamic loading when zooming).
 */
private suspend fun loadTilesForZoom(
    center: LatLon,
    zoom: Int,
    diskCache: TileCache
): Map<String, ImageBitmap> = withContext(Dispatchers.IO) {
    val cache = mutableMapOf<String, ImageBitmap>()
    val range = when (zoom) {
        in 0..12 -> 3
        in 13..14 -> 3
        in 15..16 -> 3
        in 17..18 -> 4
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
                bitmap?.let {
                    cache[key] = it.asImageBitmap()
                }
            }
        }
    }

    cache
}
