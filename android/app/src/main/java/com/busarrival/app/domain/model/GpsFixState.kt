package com.busarrival.app.domain.model

sealed class GpsFixState {
    data object NoSignal : GpsFixState()
    data object Searching : GpsFixState()
    data class Acquiring(val satellites: Int) : GpsFixState()
    data class Ready(
        val accuracyM: Float,
        val satellites: Int,
        val bearing: Float?
    ) : GpsFixState()
}
