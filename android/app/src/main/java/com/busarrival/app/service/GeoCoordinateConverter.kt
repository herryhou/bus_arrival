package com.busarrival.app.service

import com.busarrival.app.data.pipeline.types.GeoCdeg
import com.busarrival.app.domain.model.RouteData
import kotlin.math.cos
import kotlin.math.sin

/**
 * Convert between geographic coordinates and route grid coordinates.
 * Uses the route's origin as reference point.
 */
object GeoCoordinateConverter {

    /**
     * Convert lat/lon (in degrees) to grid coordinates (cm) relative to route origin.
     * Uses trigonometric formula matching Rust preprocessor (coord.rs).
     */
    fun toGridCoordinates(
        lat: Double,  // Latitude in degrees
        lon: Double,  // Longitude in degrees
        routeData: RouteData
    ): Pair<Int, Int> {
        // Calculate offsets using trigonometric formula (matches Rust)
        // Reference: crates/preprocessor/src/coord.rs:latlon_to_cm_relative
        val R_CM = 637100000.0  // Earth radius in cm
        val FIXED_ORIGIN_LAT_DEG = 20.0
        val FIXED_ORIGIN_LON_DEG = 120.0

        // Calculate average latitude from route data (or use default if corrupted)
        val avgLat = kotlin.math.max(routeData.avgLat / 1e6, 24.0)  // Use min 24° if corrupted
        val avgLatRad = Math.toRadians(avgLat)
        val cosLat = kotlin.math.cos(avgLatRad)

        // Absolute positions (in cm)
        val latRad = Math.toRadians(lat)
        val lonRad = Math.toRadians(lon)

        val xAbs = R_CM * lonRad * cosLat
        val yAbs = R_CM * latRad

        // Origin reference point (uses FIXED origin, NOT avgLat)
        // IMPORTANT: y0Abs uses FIXED_ORIGIN_LAT_DEG, not avgLat!
        val x0Abs = (Math.toRadians(FIXED_ORIGIN_LON_DEG) * R_CM) * cosLat
        val y0Abs = R_CM * Math.toRadians(FIXED_ORIGIN_LAT_DEG)  // Fixed 20°N

        // Relative coordinates (in cm, rounded to match Rust)
        val xCm = kotlin.math.round(xAbs - x0Abs).toInt()
        val yCm = kotlin.math.round(yAbs - y0Abs).toInt()

        return Pair(xCm, yCm)
    }

    /**
     * Project grid coordinates onto route to find progress (sCm).
     * This is a simplified version - full implementation would use
     * the spatial grid for fast segment lookup.
     */
    fun projectToRoute(
        xCm: Int,
        yCm: Int,
        routeData: RouteData,
        lastSegIdx: Int
    ): Pair<Int, Int> {
        // Find closest segment
        var bestIdx = lastSegIdx
        var bestDist = Int.MAX_VALUE
        var bestProgress = 0

        // Search nearby segments (limited range for performance)
        val searchRange = 50
        val startIdx = (lastSegIdx - searchRange).coerceAtLeast(0)
        val endIdx = (lastSegIdx + searchRange).coerceAtMost(routeData.nodes.size - 2)

        for (i in startIdx..endIdx) {
            val node = routeData.nodes[i]
            val nextNode = routeData.nodes[i + 1]

            // Simple point-to-segment distance
            val result = pointToSegmentDistance(xCm, yCm, node.xCm, node.yCm, nextNode.xCm, nextNode.yCm)

            if (result.first < bestDist) {
                bestDist = result.first
                bestIdx = i
                bestProgress = result.second
            }
        }

        // Calculate sCm from segment index and progress
        val baseSCm = routeData.nodes[bestIdx].cumDistCm
        val sCm = baseSCm + bestProgress

        return Pair(sCm, bestIdx)
    }

    /**
     * Calculate distance from point to line segment and projection progress.
     * Returns Pair(distance, progressAlongSegment).
     */
    private fun pointToSegmentDistance(
        px: Int, py: Int,
        x1: Int, y1: Int,
        x2: Int, y2: Int
    ): Pair<Int, Int> {
        val dx = x2 - x1
        val dy = y2 - y1

        if (dx == 0 && dy == 0) {
            // Segment is a point
            val dist = ((px - x1) * (px - x1) + (py - y1) * (py - y1))
            return Pair(kotlin.math.sqrt(dist.toDouble()).toInt(), 0)
        }

        // Parameter t of projection onto line (clamped to [0,1])
        val t = ((px - x1) * dx + (py - y1) * dy).toFloat() / (dx * dx + dy * dy).toFloat()
        val tClamped = t.coerceIn(0f, 1f)

        // Closest point on segment
        val closestX = x1 + (tClamped * dx).toInt()
        val closestY = y1 + (tClamped * dy).toInt()

        val dist = ((px - closestX) * (px - closestX) + (py - closestY) * (py - closestY))
        val progress = (tClamped * kotlin.math.sqrt((dx * dx + dy * dy).toDouble())).toInt()

        return Pair(kotlin.math.sqrt(dist.toDouble()).toInt(), progress)
    }
}
