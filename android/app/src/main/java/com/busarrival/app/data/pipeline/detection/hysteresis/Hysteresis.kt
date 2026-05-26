package com.busarrival.app.data.pipeline.detection.hysteresis

import com.busarrival.app.data.pipeline.types.*

/**
 * Off-route hysteresis state machine.
 * Ported from crates/pipeline/gps_processor/src/kalman/hysteresis.rs:44-96
 *
 * Key behaviors:
 * - Freeze position on FIRST suspect tick (suspectTicks == 0)
 * - Do NOT clear frozenSCm when returning to Normal (snap logic handles this)
 * - Suspect status also skips projection/Kalman (not just OffRoute)
 */
object Hysteresis {
    const val OFF_ROUTE_D2_THRESHOLD: Dist2 = 25000000L  // 50m²
    const val OFF_ROUTE_CONFIRM_TICKS: UByte = 5u
    const val OFF_ROUTE_CLEAR_TICKS: UByte = 2u

    data class State(
        val suspectTicks: UByte = 0u,
        val clearTicks: UByte = 0u,
        val frozenSCm: DistCm? = null,
        val freezeTime: TimestampMs? = null
    )

    data class Result(
        val status: Status,
        val state: State
    )

    enum class Status { Normal, Suspect, OffRoute }

    /**
     * Update hysteresis state based on match quality.
     *
     * Key semantic differences from Android's previous ModeMachine:
     * 1. Freeze on FIRST suspect tick (suspectTicks == 0), not on OffRoute confirmed
     * 2. Normal status does NOT clear frozenSCm - snap logic handles clearing after re-entry
     * 3. Suspect status also returns early, skipping projection/Kalman
     *
     * Rust: hysteresis.rs:52-56 (freeze logic), hysteresis.rs:82-87 (Normal doesn't clear)
     */
    fun update(
        matchDist2: Dist2,
        lastState: State,
        currentSCm: DistCm,
        currentTime: TimestampMs
    ): Result {
        val isOffRoute = matchDist2 > OFF_ROUTE_D2_THRESHOLD

        return when {
            isOffRoute -> {
                // CRITICAL: Freeze on FIRST suspect tick (suspectTicks == 0)
                // Rust: hysteresis.rs:52-56
                val newFrozenSCm = if (lastState.suspectTicks.toInt() == 0) currentSCm else lastState.frozenSCm
                val newFreezeTime = if (lastState.suspectTicks.toInt() == 0) currentTime else lastState.freezeTime

                val newSuspectTicks = lastState.suspectTicks.inc()
                val status = if (newSuspectTicks >= OFF_ROUTE_CONFIRM_TICKS) Status.OffRoute else Status.Suspect

                Result(status, lastState.copy(
                    suspectTicks = newSuspectTicks,
                    clearTicks = 0u,
                    frozenSCm = newFrozenSCm,
                    freezeTime = newFreezeTime
                ))
            }
            else -> {
                // Good match: increment clear counter
                // Rust: hysteresis.rs:73-94
                val newClearTicks = lastState.clearTicks.inc()

                if (newClearTicks >= OFF_ROUTE_CLEAR_TICKS) {
                    // CRITICAL: Return Normal but DO NOT clear frozenSCm
                    // Snap logic handles clearing after successful re-entry
                    // Rust: hysteresis.rs:82-87 (line 84 comment)
                    Result(Status.Normal, lastState.copy(
                        suspectTicks = 0u,
                        clearTicks = newClearTicks
                        // frozenSCm NOT cleared here
                    ))
                } else {
                    // Still suspect (need more good ticks to clear)
                    val isActuallySuspect = lastState.suspectTicks > 0u || lastState.frozenSCm != null
                    val status = if (isActuallySuspect) Status.Suspect else Status.Normal
                    Result(status, lastState.copy(
                        clearTicks = newClearTicks
                    ))
                }
            }
        }
    }
}
