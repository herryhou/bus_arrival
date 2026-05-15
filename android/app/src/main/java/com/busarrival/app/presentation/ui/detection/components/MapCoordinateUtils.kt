package com.busarrival.app.presentation.ui.detection.components

import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.atan
import kotlin.math.exp
import kotlin.math.pow
import kotlin.math.tan

/**
 * Coordinate conversion utilities for map rendering. Top-level functions for testability (not local
 * to drawScope).
 */

/**
 * Convert longitude to world X coordinate at given zoom level. Uses tileZ (NOT baseZ) to ensure
 * tiles align correctly.
 */
fun worldX(lon: Double, centerLon: Double, tileZ: Int): Float {
    return lonToPixelX(lon, tileZ) - lonToPixelX(centerLon, tileZ)
}

/**
 * Convert latitude to world Y coordinate at given zoom level. Uses tileZ (NOT baseZ) to ensure
 * tiles align correctly.
 */
fun worldY(lat: Double, centerLat: Double, tileZ: Int): Float {
    return latToPixelY(lat, tileZ) - latToPixelY(centerLat, tileZ)
}

/** Convert longitude to pixel X at given zoom level (Web Mercator). */
fun lonToPixelX(lon: Double, zoom: Int): Float {
    val x = (lon + 180.0) / 360.0 * 2.0.pow(zoom)
    return (x * 256).toFloat()
}

/** Convert latitude to pixel Y at given zoom level (Web Mercator). */
fun latToPixelY(lat: Double, zoom: Int): Float {
    val y = (1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom)
    return (y * 256).toFloat()
}

/** Convert longitude to tile X coordinate at given zoom level. */
fun lonToTileX(lon: Double, zoom: Int): Int {
    return ((lon + 180.0) / 360.0 * 2.0.pow(zoom)).toInt()
}

/** Convert latitude to tile Y coordinate at given zoom level. */
fun latToTileY(lat: Double, zoom: Int): Int {
    return ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(zoom)).toInt()
}

/** Convert tile X to longitude at given zoom level. */
fun tileXToLon(tileX: Int, zoom: Int): Double {
    return tileX / 2.0.pow(zoom) * 360.0 - 180.0
}

/** Convert tile Y to latitude at given zoom level. */
fun tileYToLat(tileY: Int, zoom: Int): Double {
    val n = PI - 2.0 * PI * tileY / 2.0.pow(zoom)
    return 180.0 / PI * atan(0.5 * (exp(n) - exp(-n)))
}
