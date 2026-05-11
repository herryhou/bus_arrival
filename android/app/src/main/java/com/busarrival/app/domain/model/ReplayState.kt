package com.busarrival.app.domain.model

/**
 * Timeline/replay state for historical inspection.
 */
data class ReplayState(
    val currentTime: Long = 0,        // Current position in trace (ms)
    val isPlaying: Boolean = false,   // Play/pause state
    val playbackSpeed: Float = 1f,    // 0.5x, 1x, 2x, 4x
    val traceDuration: Long = 0,      // Total trace length (ms)
    val cameraFollowEnabled: Boolean = true,
    val traceFile: String? = null     // Currently loaded trace path
)