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
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.URL
import kotlin.math.PI
import kotlin.math.atan
import kotlin.math.asinh
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.tan

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
    viewModel: DetectionViewModel,
    modifier: Modifier = Modifier
) {
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
            val tiles = preloadTilesSync(centerLatLon)
            android.util.Log.d("MapView", "Preloaded ${tiles.size} tiles")
            viewModel.addTiles(tiles)
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
                val tileZ = 15

                withTransform({
                    translate(left = offset.x, top = offset.y)
                    scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
                }) {
                    // Draw tiles using same coordinate system as route
                    val centerPixelX = lonToPixelX(center.lon, tileZ)
                    val centerPixelY = latToPixelY(center.lat, tileZ)

                    val centerTileX = lonToTileX(center.lon, tileZ)
                    val centerTileY = latToTileY(center.lat, tileZ)
                    val tileSize = 256f

                    for (dx in -3..3) {
                        for (dy in -3..3) {
                            val tileX = centerTileX + dx
                            val tileY = centerTileY + dy
                            val key = "$tileZ/$tileX/$tileY"

                            tileCache[key]?.let { bitmap ->
                                // Position tiles using Web Mercator pixel conversion
                                // Calculate tile's northwest corner lat/lon
                                val tileNW = tileXToLon(tileX, tileZ)
                                val tileNE = tileYToLat(tileY, tileZ)

                                // Convert to pixels using same formulas as route
                                val tilePixelX = lonToPixelX(tileNW, tileZ)
                                val tilePixelY = latToPixelY(tileNE, tileZ)

                                // Position relative to center
                                val tileScreenX = tilePixelX - centerPixelX + canvasWidth / 2
                                val tileScreenY = tilePixelY - centerPixelY + canvasHeight / 2

                                drawImage(
                                    image = bitmap,
                                    topLeft = Offset(tileScreenX, tileScreenY)
                                )
                            }
                        }
                    }

                    // Convert route nodes to screen coordinates
                    val path = Path()
                    routeData.nodes.forEachIndexed { index, node ->
                        val ll = routeData.cmToLatLon(node.xCm, node.yCm)
                        val px = lonToPixelX(ll.lon, tileZ) - centerPixelX + canvasWidth / 2
                        val py = latToPixelY(ll.lat, tileZ) - centerPixelY + canvasHeight / 2

                        if (index == 0) {
                            path.moveTo(px, py)
                        } else {
                            path.lineTo(px, py)
                        }
                    }

                    drawPath(
                        path = path,
                        color = Color.Blue,
                        style = Stroke(width = 4f)
                    )

                    routeData.stops.forEach { stop ->
                        val node = routeData.findNodeAtProgress(stop.progressCm)
                        if (node != null) {
                            val ll = routeData.cmToLatLon(node.xCm, node.yCm)
                            val px = lonToPixelX(ll.lon, tileZ) - centerPixelX + canvasWidth / 2
                            val py = latToPixelY(ll.lat, tileZ) - centerPixelY + canvasHeight / 2
                            drawCircle(
                                color = Color.Red,
                                radius = 8f,
                                center = Offset(px, py)
                            )
                        }
                    }

                    val currentNode = routeData.findNodeAtProgress(currentSCm)
                    if (currentNode != null) {
                        val ll = routeData.cmToLatLon(currentNode.xCm, currentNode.yCm)
                        val px = lonToPixelX(ll.lon, tileZ) - centerPixelX + canvasWidth / 2
                        val py = latToPixelY(ll.lat, tileZ) - centerPixelY + canvasHeight / 2
                        drawCircle(
                            color = Color.Green,
                            radius = 12f,
                            center = Offset(px, py)
                        )
                    }
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
                Text(
                    text = "Progress: ${currentSCm / 100}m | Zoom: ${"%.2f".format(scale)}x",
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

private suspend fun preloadTiles(center: LatLon, cache: MutableMap<String, ImageBitmap>) = withContext(Dispatchers.IO) {
    val R_CM = 637100000.0
    val originLat = 20.0
    val originLon = 120.0
    val latAvg = 24.99008254

    val latAvgRad = latAvg * PI / 180.0
    val cosLat = cos(latAvgRad)

    for (z in 14..16) {
        val tileX = lonToTileX(center.lon, z)
        val tileY = latToTileY(center.lat, z)
        for (dx in -2..2) {
            for (dy in -2..2) {
                val key = "$z/${tileX + dx}/${tileY + dy}"
                if (!cache.containsKey(key)) {
                    try {
                        val url = URL("https://tile.openstreetmap.org/$key.png")
                        val connection = url.openConnection()
                        connection.connectTimeout = 5000
                        connection.readTimeout = 5000
                        connection.addRequestProperty("User-Agent", "BusArrival/1.0")
                        val bitmap = android.graphics.BitmapFactory.decodeStream(connection.getInputStream())
                        bitmap?.let {
                            cache[key] = it.asImageBitmap()
                        }
                    } catch (e: Exception) {
                        android.util.Log.w("MapView", "Failed to load tile $key", e)
                    }
                }
            }
        }
    }
}

// Sync version for LaunchedEffect (run in coroutine)
private suspend fun preloadTilesSync(center: LatLon): Map<String, ImageBitmap> {
    val cache = mutableMapOf<String, ImageBitmap>()
    preloadTiles(center, cache)
    return cache
}

// Tile coordinate conversions
private fun latToTileY(lat: Double, zoom: Int): Int {
    return ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom)).toInt()
}

private fun lonToTileX(lon: Double, zoom: Int): Int {
    return ((lon + 180.0) / 360.0 * 2.0.pow(zoom)).toInt()
}

// Pixel coordinate conversions (for drawing)
private fun latToPixelY(lat: Double, zoom: Int): Float {
    val y = ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom))
    return (y * 256).toFloat()
}

private fun lonToPixelX(lon: Double, zoom: Int): Float {
    val x = ((lon + 180.0) / 360.0 * 2.0.pow(zoom))
    return (x * 256).toFloat()
}

// Reverse conversions (for tile positioning)
private fun tileYToLat(tileY: Int, zoom: Int): Double {
    val n = PI - 2.0 * PI * tileY / 2.0.pow(zoom)
    return 180.0 / PI * atan(0.5 * (exp(n) - exp(-n)))
}

private fun tileXToLon(tileX: Int, zoom: Int): Double {
    return tileX / 2.0.pow(zoom) * 360.0 - 180.0
}
