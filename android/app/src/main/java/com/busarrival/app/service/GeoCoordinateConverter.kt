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
     * Convert lat/lon to grid coordinates (cm) relative to route origin.
     */
    fun toGridCoordinates(
        lat: GeoCdeg,
        lon: GeoCdeg,
        routeData: RouteData
    ): Pair<Int, Int> {
        // Convert to degrees
        val latDeg = lat / 100.0
        val lonDeg = lon / 100.0

        val originLatDeg = routeData.originLat / 1e6
        val originLonDeg = routeData.originLon / 1e6

        // Calculate offsets (using average latitude for longitude scaling)
        val avgLat = routeData.avgLat / 1e6

        // 1 degree latitude ≈ 1111110 cm (at equator)
        // 1 degree longitude ≈ 1111110 * cos(lat) cm
        val latScale = 1111110.0
        val lonScale = 1111110.0 * cos(Math.toRadians(avgLat))

        val xCm = ((lonDeg - originLonDeg) * lonScale).toInt()
        val yCm = ((latDeg - originLatDeg) * latScale).toInt()

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
