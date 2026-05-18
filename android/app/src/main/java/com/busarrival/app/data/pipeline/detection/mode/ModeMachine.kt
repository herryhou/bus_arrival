package com.busarrival.app.data.pipeline.detection.mode

import com.busarrival.app.data.pipeline.types.DistCm
import com.busarrival.app.data.pipeline.types.Dist2
import kotlin.math.abs

/**
 * Mode state machine for off-route detection and recovery.
 * Ported from crates/pico2-firmware/src/control/mode.rs
 *
 * States:
 * - Normal: Standard GPS tracking with arrival detection enabled
 * - OffRoute: GPS diverged from route, position frozen, detection suppressed
 * - Recovering: Active recovery in progress, searching for correct position
 *
 * Reference: specs/06-mode_machine.md
 */
object ModeMachine {

    // Off-route detection thresholds
    // Note: matchDist2 from MapMatcher is in cm²
    private const val OFF_ROUTE_D2_THRESHOLD: Dist2 = 25_000_000L    // 50m² (5000cm)² - matches Rust
    private const val OFF_ROUTE_SUSPECT_TICKS: Int = 5               // Ticks to trigger OffRoute - matches Rust
    private const val OFF_ROUTE_CLEAR_TICKS: Int = 2                 // Ticks to clear OffRoute - matches Rust
    private const val RECOVERY_DISPLACEMENT_CM: DistCm = 5000        // 50m displacement trigger (in cm)

    /**
     * Update mode state based on map match result.
     *
     * @param state Current mode state
     * @param matchDist2 Map match distance² (indicates divergence from route)
     * @param sCm Current route position (cm)
     * @param isFirstFix True if this is the first GPS fix (warmup period)
     * @return Updated mode state
     */
    fun update(
        state: ModeState,
        matchDist2: Dist2,
        sCm: DistCm,
        isFirstFix: Boolean = false
    ): ModeState {
        return when (state.mode) {
            Mode.Normal -> updateNormal(state, matchDist2, sCm, isFirstFix)
            Mode.OffRoute -> updateOffRoute(state, matchDist2, sCm)
            Mode.Recovering -> updateRecovering(state)
        }
    }

    /**
     * Update Normal mode.
     * Transition to OffRoute if divergence > 50m for 5 consecutive ticks.
     * During warmup (isFirstFix=true), off-route detection is disabled.
     */
    private fun updateNormal(
        state: ModeState,
        matchDist2: Dist2,
        sCm: DistCm,
        isFirstFix: Boolean
    ): ModeState {
        // Warmup guard: skip off-route detection on first fix
        // Matches Rust: if !is_first_fix { check_off_route }
        if (isFirstFix) {
            return state.copy(suspectTicks = 0)
        }

        val isOffRoute = matchDist2 > OFF_ROUTE_D2_THRESHOLD

        return if (isOffRoute) {
            val newSuspectTicks = state.suspectTicks + 1
            if (newSuspectTicks >= OFF_ROUTE_SUSPECT_TICKS) {
                // Transition to OffRoute: freeze position
                state.copy(
                    mode = Mode.OffRoute,
                    suspectTicks = 0,
                    clearTicks = 0,
                    frozenSCm = sCm
                )
            } else {
                state.copy(suspectTicks = newSuspectTicks)
            }
        } else {
            // Clear suspect counter if GPS is good
            state.copy(suspectTicks = 0)
        }
    }

    /**
     * Update OffRoute mode.
     * Transition to:
     * - Normal: if divergence ≤ 50m for 2 ticks AND displacement ≤ 50m
     * - Recovering: if divergence ≤ 50m for 2 ticks AND displacement > 50m
     */
    private fun updateOffRoute(
        state: ModeState,
        matchDist2: Dist2,
        sCm: DistCm
    ): ModeState {
        val isOnRoute = matchDist2 <= OFF_ROUTE_D2_THRESHOLD

        return if (isOnRoute) {
            val newClearTicks = state.clearTicks + 1
            if (newClearTicks >= OFF_ROUTE_CLEAR_TICKS) {
                // Check displacement from frozen position
                val displacement = abs(sCm - state.frozenSCm)
                if (displacement > RECOVERY_DISPLACEMENT_CM) {
                    // Large displacement: need recovery
                    state.copy(
                        mode = Mode.Recovering,
                        clearTicks = 0
                    )
                } else {
                    // Small displacement: return to normal
                    state.copy(
                        mode = Mode.Normal,
                        clearTicks = 0,
                        frozenSCm = 0
                    )
                }
            } else {
                state.copy(clearTicks = newClearTicks)
            }
        } else {
            // Still off route: reset clear counter
            state.copy(clearTicks = 0)
        }
    }

    /**
     * Update Recovering mode.
     * Stays in Recovering until explicitly transitioned by DetectionPipeline
     * after successful recovery via Recovery.recover().
     */
    private fun updateRecovering(state: ModeState): ModeState {
        // Stay in Recovering mode until DetectionPipeline
        // successfully recovers and transitions to Normal
        return state
    }

    /**
     * Force transition to Normal mode (called after successful recovery).
     */
    fun toNormal(): ModeState {
        return ModeState(mode = Mode.Normal)
    }
}

/**
 * Operating mode.
 */
enum class Mode {
    Normal,      // Standard GPS tracking
    OffRoute,    // GPS diverged from route
    Recovering   // Re-acquiring position
}

/**
 * Mode state machine state.
 */
data class ModeState(
    val mode: Mode = Mode.Normal,
    val suspectTicks: Int = 0,    // Consecutive high-divergence ticks
    val clearTicks: Int = 0,      // Consecutive low-divergence ticks
    val frozenSCm: DistCm = 0     // Frozen position during OffRoute
)
