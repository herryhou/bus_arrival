package com.busarrival.app.domain.model

import com.busarrival.app.data.pipeline.types.*

/**
 * Route data models.
 * Ported from crates/shared/src/lib.rs
 */

/**
 * Route node (waypoint) along the route.
 * Size: 24 bytes (matching Rust layout).
 */
data class RouteNode(
    val xCm: DistCm,        // X coordinate (cm)
    val yCm: DistCm,        // Y coordinate (cm)
    val cumDistCm: DistCm,  // Cumulative distance (cm)
    val segLenMm: Int,      // Segment length (mm)
    val dxCm: Short,        // Segment vector X (cm)
    val dyCm: Short,        // Segment vector Y (cm)
    val headingCdeg: HeadCdeg, // Segment heading
    val pad: Short = 0      // Alignment padding
)

/**
 * Bus stop along the route.
 * Size: 12 bytes.
 */
data class Stop(
    val progressCm: DistCm,       // Position along route (cm)
    val corridorStartCm: DistCm, // -80 m from stop
    val corridorEndCm: DistCm    // +40 m from stop
) {
    /**
     * Check if a position is within this stop's corridor.
     */
    fun isInCorridor(sCm: DistCm): Boolean {
        return sCm in corridorStartCm..corridorEndCm
    }

    /**
     * Distance to stop from current position.
     */
    fun distanceTo(sCm: DistCm): DistCm {
        return sCm - progressCm
    }
}

/**
 * Complete route data.
 */
data class RouteData(
    val originLat: Int,      // Grid origin latitude
    val originLon: Int,      // Grid origin longitude
    val avgLat: Int,         // Average latitude for scale
    val x0Cm: Int = 0,       // Grid origin X offset (cm)
    val y0Cm: Int = 0,       // Grid origin Y offset (cm)
    val nodes: List<RouteNode>,
    val stops: List<Stop>,
    val grid: SpatialGrid
)

/**
 * Spatial grid for fast segment lookup (v5.1 sparse format).
 */
data class SpatialGrid(
    val cellSizeCm: DistCm,
    val rows: Int,
    val cols: Int,
    val cells: List<GridCell>
)

/**
 * Single grid cell containing segment indices.
 */
data class GridCell(
    val bitmask: ULong,      // 64-bit bitmask for dense segments
    val offsets: List<Int>   // Sparse offsets for additional segments
)

/**
 * Route metadata for storage manager.
 */
data class RouteMetadata(
    val uuid: String,
    val name: String,
    val timestamp: Long,
    val stopCount: Int,
    val filePath: String
)

/**
 * Detection parameters for probability model.
 */
data class DetectionParameters(
    val distanceWeight: Int = 50,
    val speedWeight: Int = 50,
    val progressErrorWeight: Int = 50,
    val dwellTimeWeight: Int = 50,
    val corridorSize: Int = 0  // -80m to +40m range, 0 = default
) {
    companion object {
        val defaults = DetectionParameters()
    }
}
