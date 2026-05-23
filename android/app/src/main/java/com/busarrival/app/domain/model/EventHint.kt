package com.busarrival.app.domain.model

data class EventHint(
    val type: HintType,
    val stopIndex: Int,
    val timestamp: Long = System.currentTimeMillis()
)

enum class HintType {
    APPROACHING,
    ARRIVING,
    ATSTOP,
    DEPART
}
