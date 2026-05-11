package com.busarrival.app.domain.model

import androidx.compose.ui.geometry.Offset

/**
 * Unified map rendering state.
 * Consolidates scale, offset, and center coordinates.
 */
data class MapState(
    val centerLon: Double = 120.0,
    val centerLat: Double = 20.0,
    val tileZ: Int = 15,
    val scale: Float = 1f,
    val offset: Offset = Offset.Zero
)