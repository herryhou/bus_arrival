package com.busarrival.app.data.pipeline.localization.mapmatcher

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/**
 * Heading-constrained map matching.
 * Ported from crates/pipeline/gps_processor/src/map_match.rs
 *
 * Filter-then-Rank architecture:
 * 1. Filter by heading (±90° gate)
 * 2. Rank by distance (closest segment wins)
 *
 * Reference: specs/01-map_matching.md
 */
object MapMatcher {

    private const val MAX_HEADING_DIFF_CDEG = 9000  // 90°
    private const val SIGMA_GPS_CM: DistCm = 2000
    private const val MAX_DIST2_EARLY_EXIT: Dist2 = 4_000_000L  // SIGMA_GPS_CM²

    private const val WINDOW_BACK = 2
    private const val WINDOW_FWD = 10

    /**
     * Map matching result.
     */
    data class MatchResult(
        val segIdx: Int,
        val dist2: Dist2
    )

    /**
     * Find best matching segment for GPS position.
     *
     * @param gpsX GPS X coordinate (cm)
     * @param gpsY GPS Y coordinate (cm)
     * @param gpsHeading GPS heading (0.01°), null if unknown
     * @param gpsSpeed GPS speed (cm/s)
     * @param routeData Route data with spatial grid
     * @param lastIdx Previous segment index
     * @param isFirstFix True for first fix after outage
     * @return MatchResult with segment index and distance²
     */
    fun match(
        gpsX: DistCm,
        gpsY: DistCm,
        gpsHeading: HeadCdeg?,
        gpsSpeed: SpeedCms,
        routeData: RouteData,
        lastIdx: Int,
        isFirstFix: Boolean
    ): MatchResult {
        val nodes = routeData.nodes
        if (nodes.isEmpty()) {
            return MatchResult(0, Long.MAX_VALUE)
        }

        // Phase 1: Window search
        val windowResult = searchWindow(
            gpsX, gpsY, gpsHeading, gpsSpeed,
            nodes, lastIdx, isFirstFix
        )

        if (windowResult.earlyExit) {
            return MatchResult(windowResult.bestIdx, windowResult.bestDist2)
        }

        // Phase 2: Grid search (if needed)
        val gridResult = searchGrid(
            gpsX, gpsY, gpsHeading, gpsSpeed,
            routeData, nodes,
            windowResult.bestIdx, windowResult.bestDist2, windowResult.eligibleFound
        )

        return MatchResult(gridResult.bestIdx, gridResult.bestDist2)
    }

    /**
     * Window search: last_idx ±2/+10.
     */
    private fun searchWindow(
        gpsX: DistCm,
        gpsY: DistCm,
        gpsHeading: HeadCdeg?,
        gpsSpeed: SpeedCms,
        nodes: List<RouteNode>,
        lastIdx: Int,
        isFirstFix: Boolean
    ): SearchResult {
        var bestIdx = 0
        var bestDist2: Dist2 = Long.MAX_VALUE
        var eligibleFound = false
        var earlyExit = false

        val startIdx = max(0, lastIdx - WINDOW_BACK)
        val endIdx = min(nodes.size - 1, lastIdx + WINDOW_FWD)

        val headingThresh = headingThreshold(gpsSpeed, isFirstFix)

        for (i in startIdx..endIdx) {
            val node = nodes[i]
            val eligible = isHeadingEligible(
                gpsHeading, node.headingCdeg, headingThresh
            )

            val dist2 = pointToSegmentDist2(gpsX, gpsY, node)

            if (eligible) {
                eligibleFound = true
                if (dist2 < bestDist2) {
                    bestIdx = i
                    bestDist2 = dist2
                    if (dist2 < MAX_DIST2_EARLY_EXIT) {
                        earlyExit = true
                        break
                    }
                }
            } else if (!eligibleFound && dist2 < bestDist2) {
                // Keep track of best ineligible segment
                bestIdx = i
                bestDist2 = dist2
            }
        }

        return SearchResult(bestIdx, bestDist2, eligibleFound, earlyExit)
    }

    /**
     * Grid search: 3×3 cell neighborhood.
     */
    private fun searchGrid(
        gpsX: DistCm,
        gpsY: DistCm,
        gpsHeading: HeadCdeg?,
        gpsSpeed: SpeedCms,
        routeData: RouteData,
        nodes: List<RouteNode>,
        seedIdx: Int,
        seedDist2: Dist2,
        seedEligibleFound: Boolean
    ): SearchResult {
        val grid = routeData.grid
        val cellX = (gpsX - routeData.x0Cm) / grid.cellSizeCm
        val cellY = (gpsY - routeData.y0Cm) / grid.cellSizeCm

        var bestIdx = seedIdx
        var bestDist2 = seedDist2
        var eligibleFound = seedEligibleFound

        val headingThresh = headingThreshold(gpsSpeed, false)

        // Search 3×3 neighborhood
        for (dy in -1..1) {
            for (dx in -1..1) {
                val cx = cellX + dx
                val cy = cellY + dy

                if (cx < 0 || cx >= grid.cols || cy < 0 || cy >= grid.rows) continue

                val cellIdx = cy * grid.cols + cx
                val cell = grid.cells[cellIdx]

                // Check bitmask segments
                for (bit in 0..63) {
                    if ((cell.bitmask and (1UL shl bit)) != 0UL) {
                        val segIdx = cellIdx * 64 + bit
                        if (segIdx >= nodes.size) break

                        val eligible = isHeadingEligible(gpsHeading, nodes[segIdx].headingCdeg, headingThresh)
                        if (eligible) {
                            val dist2 = pointToSegmentDist2(gpsX, gpsY, nodes[segIdx])
                            if (dist2 < bestDist2) {
                                bestIdx = segIdx
                                bestDist2 = dist2
                                eligibleFound = true
                            }
                        }
                    }
                }

                // Check sparse offsets
                for (offset in cell.offsets) {
                    val segIdx = cellIdx * 64 + offset
                    if (segIdx >= nodes.size) break

                    val eligible = isHeadingEligible(gpsHeading, nodes[segIdx].headingCdeg, headingThresh)
                    if (eligible) {
                        val dist2 = pointToSegmentDist2(gpsX, gpsY, nodes[segIdx])
                        if (dist2 < bestDist2) {
                            bestIdx = segIdx
                            bestDist2 = dist2
                            eligibleFound = true
                        }
                    }
                }
            }
        }

        return SearchResult(bestIdx, bestDist2, eligibleFound, false)
    }

    private fun checkSegment(
        node: RouteNode,
        gpsX: DistCm,
        gpsY: DistCm,
        gpsHeading: HeadCdeg?,
        headingThresh: Int,
        segIdx: Int,
        onUpdate: (Int, Dist2, Boolean) -> Unit
    ) {
        val eligible = isHeadingEligible(gpsHeading, node.headingCdeg, headingThresh)
        val dist2 = pointToSegmentDist2(gpsX, gpsY, node)

        if (eligible) {
            onUpdate(segIdx, dist2, true)
        }
    }

    /**
     * Check if segment is heading-eligible.
     */
    private fun isHeadingEligible(
        gpsHeading: HeadCdeg?,
        segHeading: HeadCdeg,
        threshold: Int
    ): Boolean {
        if (gpsHeading == null) return true  // No heading info
        if (threshold == Int.MAX_VALUE) return true  // Standstill or first fix

        val diff = headingDiffCdeg(gpsHeading, segHeading).toInt()
        return diff <= threshold
    }

    /**
     * Calculate heading threshold based on speed.
     */
    private fun headingThreshold(speed: SpeedCms, isFirstFix: Boolean): Int {
        if (isFirstFix) return 18000  // 180° relaxed threshold

        val w = headingWeight(speed)
        if (w == 0) return Int.MAX_VALUE  // Standstill: disable heading gate

        // Threshold = 36° - (27° × w / 256)
        val range = 36000 - 9000  // 27000
        return 36000 - range * w / 256
    }

    /**
     * Calculate heading weight (0..256).
     */
    private fun headingWeight(speed: SpeedCms): Int {
        // 0 cm/s → 0, ≥83 cm/s → 256
        return (speed * 256 / 83).coerceIn(0, 256)
    }

    /**
     * Point-to-segment distance² (no sqrt).
     * Clamped projection to segment.
     */
    private fun pointToSegmentDist2(
        px: DistCm,
        py: DistCm,
        node: RouteNode
    ): Dist2 {
        // Segment vector: (dx, dy)
        // Vector from segment start to point: (px - x, py - y)
        val vx = px - node.xCm
        val vy = py - node.yCm

        // Projection t = (v · d) / |d|²
        val dot = (vx * node.dxCm + vy * node.dyCm).toLong()
        val len2 = (node.dxCm * node.dxCm + node.dyCm * node.dyCm).toLong()

        if (len2 == 0L) {
            // Zero-length segment
            return (vx * vx + vy * vy).toLong()
        }

        // Clamped projection
        val t = when {
            dot <= 0 -> 0L
            dot >= len2 -> 1L shl 32  // Represent 1.0 in fixed-point
            else -> (dot shl 32) / len2
        }

        // Closest point on segment
        val cx = node.xCm + ((node.dxCm * (t shr 16)) shr 16)  // Approximate t * dx
        val cy = node.yCm + ((node.dyCm * (t shr 16)) shr 16)

        // Distance²
        val dx = px - cx
        val dy = py - cy
        return dx.toLong() * dx + dy.toLong() * dy
    }

    /**
     * Search result holder.
     */
    private data class SearchResult(
        val bestIdx: Int,
        val bestDist2: Dist2,
        val eligibleFound: Boolean,
        val earlyExit: Boolean
    )
}
