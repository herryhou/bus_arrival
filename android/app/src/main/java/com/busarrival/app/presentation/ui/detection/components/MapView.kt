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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.busarrival.app.BuildConfig
import com.busarrival.app.data.cache.TileCache
import com.busarrival.app.data.cache.TileResolution
import com.busarrival.app.domain.model.ReplayState
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import kotlin.math.PI
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.withContext

private const val BASE_Z = 15
private const val MIN_TILE_Z = 14
private const val MAX_TILE_Z = 20
private const val TILE_REQUEST_BUCKET_WORLD_PX = 256f
private const val TILE_REQUEST_DELAY_MS = 75L
private const val TILE_PREFETCH_PADDING = 1
private const val MAX_FETCH_RANGE = 5
private const val MAX_TILE_CONCURRENCY = 4

data class LatLon(val lat: Double, val lon: Double)

fun RouteData.cmToLatLon(xCm: Int, yCm: Int): LatLon {
    // Match Rust code exactly: spherical Earth R = 6371 km
    val R_CM = 637100000.0
    val originLat = this.originLat.toDouble() / 1e6
    val originLon = this.originLon.toDouble() / 1e6
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
        gpsLat: Double,
        gpsLon: Double,
        gpsBearing: Float?,
        onToggleCameraFollow: () -> Unit = {},
        modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val tileDiskCache = remember { TileCache(context) }

    val scale by viewModel.mapScale.collectAsState()
    val offset by viewModel.mapOffset.collectAsState()
    val mapLabelZoomBias by viewModel.mapLabelZoomBias.collectAsState()
    val tileCache by viewModel.tileCache.collectAsState()
    val canvasSize = remember { androidx.compose.runtime.mutableStateOf(IntSize.Zero) }
    val showDebugDetails = remember { mutableStateOf(false) }
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

    // Camera follow: center map on current position only when near edge
    val shouldFollow = isCameraFollowEnabled || replayState.cameraFollowEnabled
    val edgeThresholdPx = 100f

    LaunchedEffect(currentSCm, shouldFollow, routeData, scale, offset, canvasSize.value) {
        if (shouldFollow && routeData != null && centerLatLon != null) {
            val size = canvasSize.value
            if (size.width <= 0 || size.height <= 0) return@LaunchedEffect

            // Get current screen position
            val pos = routeData.interpolatePosition(currentSCm)
            if (pos != null) {
                val ll = routeData.cmToLatLon(pos.first, pos.second)

                // Calculate current screen position
                val worldX = lonToPixelX(ll.lon, BASE_Z) - lonToPixelX(centerLatLon.lon, BASE_Z)
                val worldY = latToPixelY(ll.lat, BASE_Z) - latToPixelY(centerLatLon.lat, BASE_Z)
                val screenX = worldX * scale + offset.x + size.width / 2f
                val screenY = worldY * scale + offset.y + size.height / 2f

                // Check if near edge (within threshold)
                val nearLeft = screenX < edgeThresholdPx
                val nearRight = screenX > size.width - edgeThresholdPx
                val nearTop = screenY < edgeThresholdPx
                val nearBottom = screenY > size.height - edgeThresholdPx

                // Update center only when near edge
                if (nearLeft || nearRight || nearTop || nearBottom) {
                    val targetX =
                            lonToPixelX(ll.lon, BASE_Z) - lonToPixelX(centerLatLon.lon, BASE_Z)
                    val targetY =
                            latToPixelY(ll.lat, BASE_Z) - latToPixelY(centerLatLon.lat, BASE_Z)
                    viewModel.updateMapState(scale, Offset(-targetX * scale, -targetY * scale))
                }
            }
        }
    }
    // Calculate tile zoom level from scale - use derivedStateOf to ensure updates
    val tileZ by remember {
        derivedStateOf {
            (BASE_Z + (kotlin.math.ln(scale.toDouble()) / kotlin.math.ln(2.0)).toInt()).coerceIn(
                    MIN_TILE_Z,
                    MAX_TILE_Z
            )
        }
    }
    val requestedTileZ by remember {
        derivedStateOf { (tileZ - mapLabelZoomBias).coerceIn(MIN_TILE_Z, MAX_TILE_Z) }
    }

    // Viewport center in world coordinates (changes with pan)
    val viewportCenterWorld by remember {
        derivedStateOf {
            val centerX = -offset.x / scale
            val centerY = -offset.y / scale
            Pair(centerX, centerY)
        }
    }

    val viewportBucket by
            remember(requestedTileZ, viewportCenterWorld) {
                derivedStateOf {
                    val tileWorldSize = 256f * 2.0f.pow(BASE_Z - requestedTileZ)
                    val bucketSize = maxOf(tileWorldSize, TILE_REQUEST_BUCKET_WORLD_PX)
                    Pair(
                            kotlin.math.floor(viewportCenterWorld.first / bucketSize).toInt(),
                            kotlin.math.floor(viewportCenterWorld.second / bucketSize).toInt()
                    )
                }
            }

    val viewportCenterLatLon by
            remember(centerLatLon, viewportCenterWorld) {
                derivedStateOf {
                    centerLatLon?.let {
                        viewportCenterToLatLon(
                                routeCenter = it,
                                viewportCenterWorldX = viewportCenterWorld.first,
                                viewportCenterWorldY = viewportCenterWorld.second,
                                zoom = BASE_Z
                        )
                    }
                }
            }
    // Raw GPS position (for bus icon)
    val busScreenPosition by
            remember(routeData, centerLatLon, gpsLat, gpsLon, scale, offset, canvasSize.value) {
                derivedStateOf {
                    val center = centerLatLon ?: return@derivedStateOf null
                    val size = canvasSize.value
                    if (size.width <= 0 || size.height <= 0) return@derivedStateOf null
                    if (gpsLat == 0.0 || gpsLon == 0.0) return@derivedStateOf null

                    val worldX = lonToPixelX(gpsLon, BASE_Z) - lonToPixelX(center.lon, BASE_Z)
                    val worldY = latToPixelY(gpsLat, BASE_Z) - latToPixelY(center.lat, BASE_Z)
                    val x = worldX * scale + offset.x + size.width / 2f
                    val y = worldY * scale + offset.y + size.height / 2f
                    if (x.isFinite() && y.isFinite()) Offset(x, y) else null
                }
            }

    val drawTileRange by
            remember(requestedTileZ, canvasSize.value, scale) {
                derivedStateOf {
                    computeVisibleTileRange(
                            zoom = requestedTileZ,
                            canvasWidth = canvasSize.value.width.toFloat(),
                            canvasHeight = canvasSize.value.height.toFloat(),
                            scale = scale
                    )
                }
            }

    LaunchedEffect(requestedTileZ, viewportBucket, viewportCenterLatLon, drawTileRange) {
        val loadCenter = viewportCenterLatLon ?: return@LaunchedEffect

        kotlinx.coroutines.delay(TILE_REQUEST_DELAY_MS)

        val fetchRange = (drawTileRange + TILE_PREFETCH_PADDING).coerceIn(1, MAX_FETCH_RANGE)
        val resolution = TileResolution.forZoom(requestedTileZ)

        val tiles =
                kotlin
                        .runCatching {
                            loadTilesForZoom(
                                    loadCenter,
                                    requestedTileZ,
                                    tileDiskCache,
                                    fetchRange,
                                    resolution,
                                    tileCache
                            )
                        }
                        .getOrNull()

        if (!tiles.isNullOrEmpty()) {
            viewModel.addTiles(tiles)
        }
    }

    Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        if (routeData != null && centerLatLon != null) {
            Canvas(
                    modifier =
                            Modifier.fillMaxSize()
                                    .background(Color.White)
                                    .onSizeChanged { canvasSize.value = it }
                                    .pointerInput(Unit) {
                                        detectTransformGestures { centroid, pan, zoom, _ ->
                                            val oldScale = scale
                                            val newScale = (oldScale * zoom).coerceIn(0.1f, 10f)

                                            // Convert centroid from screen coordinates to centered
                                            // coordinates
                                            // Screen origin is top-left, our offset origin is
                                            // center
                                            val centroidCentered =
                                                    centroid -
                                                            Offset(
                                                                    size.width / 2f,
                                                                    size.height / 2f
                                                            )

                                            // Adjust offset to keep pinch point stable: zoom around
                                            // centroid
                                            // Formula: offset += (centroid - offset) * (1 -
                                            // newScale/oldScale)
                                            val oldOffset = offset
                                            val scaleChange = 1 - newScale / oldScale
                                            val newOffset =
                                                    oldOffset +
                                                            (centroidCentered - oldOffset) *
                                                                    scaleChange +
                                                            pan

                                            viewModel.updateMapState(newScale, newOffset)
                                        }
                                    }
            ) {
                val canvasWidth = size.width
                val canvasHeight = size.height

                val center = centerLatLon
                val viewportCenter = viewportCenterLatLon ?: center

                // Always use baseZ for positioning to prevent jumps
                val centerTileX = lonToTileX(viewportCenter.lon, requestedTileZ)
                val centerTileY = latToTileY(viewportCenter.lat, requestedTileZ)
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
                    val worldX = lonToPixelX(lon, BASE_Z) - lonToPixelX(center.lon, BASE_Z)
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
                    val worldY = latToPixelY(lat, BASE_Z) - latToPixelY(center.lat, BASE_Z)
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
                    // Compute tile range dynamically to maintain constant screen coverage
                    // tileWorldSize = 256 * 2^(baseZ - tileZ) shrinks as tileZ increases
                    val tileRange = drawTileRange

                    var tilesDrawn = 0

                    for (dx in -tileRange..tileRange) {
                        for (dy in -tileRange..tileRange) {
                            val tileX = centerTileX + dx
                            val tileY = centerTileY + dy

                            val resolution = TileResolution.forZoom(requestedTileZ)
                            val key = "$requestedTileZ/$tileX/$tileY@${resolution.scale}"

                            // Try to get tile at current zoom level
                            var bitmap = tileCache[key]
                            var actualTileZ = requestedTileZ
                            var actualTileX = tileX
                            var actualTileY = tileY
                            var actualResolution = resolution

                            // Fallback: try lower zoom levels if current not available
                            if (bitmap == null && requestedTileZ > MIN_TILE_Z) {
                                var foundFallback = false
                                for (fallbackZ in (requestedTileZ - 1) downTo MIN_TILE_Z) {
                                    if (foundFallback) break
                                    val fallbackScaleDivisor =
                                            2.0.pow(requestedTileZ - fallbackZ).toInt()
                                    val fallbackX = tileX / fallbackScaleDivisor
                                    val fallbackY = tileY / fallbackScaleDivisor
                                    val fallbackRes = TileResolution.forZoom(fallbackZ)
                                    val fallbackKey =
                                            "$fallbackZ/$fallbackX/$fallbackY@${fallbackRes.scale}"
                                    tileCache[fallbackKey]?.let {
                                        bitmap = it
                                        actualTileZ = fallbackZ
                                        actualTileX = fallbackX
                                        actualTileY = fallbackY
                                        actualResolution = fallbackRes
                                        foundFallback = true
                                    }
                                }
                            }

                            bitmap?.let {
                                // Determine which tile actually provides the bitmap
                                val posTileX =
                                        if (actualTileZ != requestedTileZ) actualTileX else tileX
                                val posTileY =
                                        if (actualTileZ != requestedTileZ) actualTileY else tileY
                                val posZ =
                                        if (actualTileZ != requestedTileZ) actualTileZ
                                        else requestedTileZ

                                // Calculate position using ACTUAL tile coordinates (posTileX,
                                // posTileY at posZ)
                                // Convert geographic to baseZ world space directly (not via
                                // worldX/Y helpers)
                                val tileNW = tileXToLon(posTileX, posZ)
                                val tileNorth = tileYToLat(posTileY, posZ)

                                // Position in baseZ world space for stable coordinates across zoom
                                // MUST use lonToPixelX/latToPixelY with baseZ directly, NOT
                                // worldX/Y helpers
                                val tileWorldX =
                                        lonToPixelX(tileNW, BASE_Z) -
                                                lonToPixelX(center.lon, BASE_Z)
                                val tileWorldY =
                                        latToPixelY(tileNorth, BASE_Z) -
                                                latToPixelY(center.lat, BASE_Z)

                                if (BuildConfig.DEBUG) {
                                    assert(tileWorldX.isFinite() && tileWorldY.isFinite()) {
                                        "Non-finite world coordinates: x=$tileWorldX, y=$tileWorldY"
                                    }
                                }

                                tilesDrawn++
                                // Scale = only the baseZ→posZ conversion; fallback bitmap naturally
                                // covers correct area
                                // Adjust for variable resolution (1x, 2x, 4x, 8x)
                                val drawScale = 2.0f.pow(BASE_Z - posZ) / actualResolution.scale

                                if (BuildConfig.DEBUG) {
                                    assert(drawScale > 0f && drawScale < 1000f) {
                                        "Invalid scale factor: $drawScale (posZ=$posZ, baseZ=$BASE_Z, resScale=${actualResolution.scale})"
                                    }
                                }

                                if (drawScale != 1f) {
                                    withTransform({
                                        scale(
                                                drawScale,
                                                drawScale,
                                                pivot = Offset(tileWorldX, tileWorldY)
                                        )
                                    }) { drawImage(it, topLeft = Offset(tileWorldX, tileWorldY)) }
                                } else {
                                    drawImage(it, topLeft = Offset(tileWorldX, tileWorldY))
                                }
                            }
                        }
                    }
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

                        // Draw 50m reference circle (semi-transparent filled disc)
                        val refRadiusM = 50f
                        val eastPoint =
                                routeData.cmToLatLon(
                                        pos.first + (refRadiusM * 100).toInt(),
                                        pos.second
                                )
                        val pxEast = toScreenX(eastPoint.lon)
                        val refRadiusPx = kotlin.math.abs(pxEast - px)
                        drawCircle(
                                color = Color.Red.copy(alpha = 0.1f),
                                radius = refRadiusPx,
                                center = Offset(px, py)
                        )
                    }
                }

                // Draw vehicle heading arrow when GPS is ready (behind snap point)
                busScreenPosition?.let { pos ->
                    val arrowSize = 48f
                    val bearing = gpsBearing ?: 0f

                    // Navigation arrow shape (standard icon, scaled to 48x48)
                    // Based on Material navigation icon, tip points up by default
                    val bearingRad = bearing * PI / 180f
                    val cosB = kotlin.math.cos(bearingRad).toFloat()
                    val sinB = kotlin.math.sin(bearingRad).toFloat()

                    // Arrow dimensions (48x48, coordinates relative to center)
                    // Tip: (0, -20) from center
                    // Outer base: (-15, 16.58) to (15, 16.58)
                    // Inner cutout: (-13.58, 18) to (0, 12) to (13.58, 18)

                    fun rotate(x: Float, y: Float): Offset {
                        // Rotate point around origin
                        val rx = x * cosB - y * sinB
                        val ry = x * sinB + y * cosB
                        return Offset(rx + pos.x, ry + pos.y)
                    }

                    val p1 = rotate(0f, -20f)
                    val p2 = rotate(-15f, 16.58f)
                    val p3 = rotate(-13.58f, 18f)
                    val p4 = rotate(0f, 12f)
                    val p5 = rotate(13.58f, 18f)
                    val p6 = rotate(15f, 16.58f)

                    val arrowPath = Path()
                    // Outer shape (clockwise from tip)
                    arrowPath.moveTo(p1.x, p1.y)
                    arrowPath.lineTo(p2.x, p2.y)
                    arrowPath.lineTo(p3.x, p3.y)
                    arrowPath.lineTo(p4.x, p4.y)
                    arrowPath.lineTo(p5.x, p5.y)
                    arrowPath.lineTo(p6.x, p6.y)
                    arrowPath.close()

                    // Draw white edge
                    drawPath(path = arrowPath, color = Color.White, style = Stroke(width = 3f))
                    // Draw filled arrow
                    drawPath(path = arrowPath, color = Color(0xFF2E7D32))
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
                                Color.Cyan // Replay mode: blue marker
                            } else {
                                Color.Green // Live mode: green marker
                            }

                    val markerRadius = 12f
                    drawCircle(
                            color = Color.White,
                            radius = markerRadius,
                            center = Offset(px, py),
                            style = Stroke(width = 3f)
                    )
                    drawCircle(
                            color = markerColor,
                            radius = markerRadius - 1.5f,
                            center = Offset(px, py)
                    )
                }
            }

            // Positioned elements layer (without center alignment affecting offsets)
            Box(modifier = Modifier.fillMaxSize()) {}

            Column(modifier = Modifier.align(Alignment.TopStart).padding(16.dp)) {
                IconButton(onClick = { showDebugDetails.value = !showDebugDetails.value }) {
                    Icon(imageVector = Icons.Default.Info, contentDescription = "Toggle debug info")
                }

                if (showDebugDetails.value) {
                    // Debug: test origin conversion
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
                            text =
                                    "Tiles: ${tileCache.size} loaded | tileZ=$requestedTileZ | labelBias=$mapLabelZoomBias",
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.Black
                    )

                    val testOrigin = routeData.cmToLatLon(0, 0)
                    Text(
                            text =
                                    "Center: ${"%.4f".format(centerLatLon.lat)}, ${"%.4f".format(centerLatLon.lon)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.Black
                    )
                    Text(
                            text =
                                    "Origin(0,0): ${"%.4f".format(testOrigin.lat)}, ${"%.4f".format(testOrigin.lon)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.Black
                    )
                }
            }

            // Camera follow toggle (top-right, mirrors info button top-left)
            Column(modifier = Modifier.align(Alignment.TopEnd).padding(16.dp)) {
                IconButton(
                        onClick = onToggleCameraFollow,
                        modifier =
                                Modifier.semantics { contentDescription = "Toggle camera follow" }
                ) {
                    Icon(
                            imageVector = Icons.Default.LocationOn,
                            contentDescription = "Toggle camera follow",
                            tint =
                                    if (isCameraFollowEnabled) {
                                        MaterialTheme.colorScheme.primary
                                    } else {
                                        MaterialTheme.colorScheme.onSurfaceVariant
                                    }
                    )
                }
            }

            Row(
                    modifier = Modifier.align(Alignment.BottomStart).padding(16.dp),
                    horizontalArrangement =
                            androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
            ) {
                androidx.compose.material3.Button(
                        onClick = {
                            viewModel.clearTileCache()
                            tileDiskCache.clearCacheAll()
                        }
                ) {
                    androidx.compose.material3.Icon(
                            imageVector = Icons.Default.Delete,
                            contentDescription = null
                    )
                    Spacer(modifier = Modifier.width(8.dp))
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

internal fun formatBusMarkerLabel(stopIndex: Int, stopState: String): String {
    val stopLabel = if (stopIndex >= 0) "Stop ${stopIndex + 1}" else "Stop -"
    val stateLabel = shortStopStateLabel(stopState)
    return "$stopLabel · $stateLabel"
}

internal fun shortStopStateLabel(stopState: String): String {
    return when (stopState.uppercase()) {
        "APPROACHING" -> "APR"
        "ARRIVING" -> "ARL"
        "ATSTOP" -> "AT"
        "DEPARTED" -> "DEP"
        "TRIPCOMPLETE" -> "DONE"
        "IDLE" -> "IDLE"
        else -> stopState
    }
}

internal fun busStateColor(stopState: String): Color {
    return when (stopState.uppercase()) {
        "APPROACHING" -> Color(0xFF1565C0)
        "ARRIVING" -> Color(0xFFF57C00)
        "ATSTOP" -> Color(0xFF2E7D32)
        "DEPARTED" -> Color(0xFF6A1B9A)
        "TRIPCOMPLETE" -> Color(0xFF424242)
        "IDLE" -> Color(0xFF37474F)
        else -> Color(0xFF263238)
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

/** Load tiles for a specific zoom level (for dynamic loading when zooming). */
private suspend fun loadTilesForZoom(
        center: LatLon,
        zoom: Int,
        diskCache: TileCache,
        tileRange: Int,
        resolution: TileResolution,
        memoryCache: Map<String, ImageBitmap>
): Map<String, ImageBitmap> =
        withContext(Dispatchers.IO) {
            // Limit concurrency to prevent crash from too many connections
            val semaphore = Semaphore(MAX_TILE_CONCURRENCY)

            val tileX = lonToTileX(center.lon, zoom)
            val tileY = latToTileY(center.lat, zoom)
            val tileCount = 1 shl zoom

            // Build list of tile coordinates first
            val tiles = mutableListOf<Pair<Int, Int>>()
            for (dx in -tileRange..tileRange) {
                for (dy in -tileRange..tileRange) {
                    val candidateY = tileY + dy
                    if (candidateY in 0 until tileCount) {
                        tiles.add(Pair(tileX + dx, candidateY))
                    }
                }
            }
            tiles.sortBy { kotlin.math.abs(it.first - tileX) + kotlin.math.abs(it.second - tileY) }

            // Load tiles in parallel with semaphore limiting concurrency
            tiles
                    .map { (x, y) ->
                        async {
                            semaphore.acquire()
                            try {
                                val key = "$zoom/$x/$y@${resolution.scale}"

                                // Skip if already in memory cache
                                if (memoryCache.containsKey(key)) {
                                    return@async null
                                }

                                // Check disk cache first
                                var bitmap = diskCache.getTile(zoom, x, y, resolution)

                                // Fetch from network if not in disk cache
                                if (bitmap == null) {
                                    bitmap = diskCache.fetchAndCacheTile(zoom, x, y, resolution)
                                }

                                // Convert to ImageBitmap for memory cache
                                bitmap?.let { key to it.asImageBitmap() }
                            } finally {
                                semaphore.release()
                            }
                        }
                    }
                    .awaitAll()
                    .filterNotNull()
                    .toMap()
        }

internal fun viewportCenterToLatLon(
        routeCenter: LatLon,
        viewportCenterWorldX: Float,
        viewportCenterWorldY: Float,
        zoom: Int
): LatLon {
    val viewportPixelX = lonToPixelX(routeCenter.lon, zoom) + viewportCenterWorldX
    val viewportPixelY = latToPixelY(routeCenter.lat, zoom) + viewportCenterWorldY
    return LatLon(lat = pixelYToLat(viewportPixelY, zoom), lon = pixelXToLon(viewportPixelX, zoom))
}

internal fun computeVisibleTileRange(
        zoom: Int,
        canvasWidth: Float,
        canvasHeight: Float,
        scale: Float
): Int {
    if (canvasWidth <= 0f || canvasHeight <= 0f || scale <= 0f || !scale.isFinite()) {
        return 2
    }

    val tileWorldSize = 256f * 2.0f.pow(BASE_Z - zoom)
    val viewportHalfWorld = maxOf(canvasWidth, canvasHeight) / 2f / scale
    return ceil(viewportHalfWorld / tileWorldSize).toInt() + 1
}
