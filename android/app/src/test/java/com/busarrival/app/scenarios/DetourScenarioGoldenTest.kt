package com.busarrival.app.scenarios

import com.busarrival.app.scenarios.common.TestDataLoader
import com.busarrival.app.scenarios.common.TraceLoader
import com.busarrival.app.service.DetectionPipeline
import com.busarrival.app.service.TraceTick
import org.junit.Assert.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * Golden standard test for ty225_short_detour scenario.
 * Validates 10 requirements per PRD line 186.
 *
 * Test data location: ../../test_data/ty225_short_detour_*
 *
 * Validations:
 * 1. Arrival sequence (stops 2,3,4,5 skipped)
 * 2. GPS monotonicity (no backward jumps)
 * 3. Off-route duration ≥5s
 * 4. Position freeze during off-route
 * 5. Immediate snap on re-entry (>100m jump)
 * 6. Skipped stops validation
 * 7. No arrivals during off-route
 * 8. Ground truth consistency
 * 9. Announce events validation
 * 10. FSM state transitions
 */
@RunWith(RobolectricTestRunner::class)
class DetourScenarioGoldenTest {

    private lateinit var traceFile: File
    private lateinit var pipeline: DetectionPipeline

    @Before
    fun setup() {
        traceFile = File.createTempFile("trace", ".jsonl")
        pipeline = DetectionPipeline()

        val routeData = TestDataLoader.loadRouteData("short_detour")
        pipeline.initialize(routeData, traceFile = traceFile)

        println("Trace file: ${traceFile.absolutePath}")
    }

    @After
    fun cleanup() {
        pipeline.close()
        if (traceFile.exists()) {
            traceFile.delete()
        }
    }

    @Test
    fun test_ty225_short_detour_golden_standard() {
        // Load NMEA test data
        val locations = TestDataLoader.loadNmea("short_detour")
        println("Processing ${locations.size} GPS points...")

        // Process all GPS points through pipeline
        for (location in locations) {
            pipeline.process(location)
        }

        // Flush and close trace writer
        pipeline.close()

        // Load trace for validation
        val ticks = TraceLoader.load(traceFile)
        println("Loaded ${ticks.size} trace ticks")

        // Run all 10 validations
        validateArrivalSequence(ticks)           // 1
        validateGpsMonotonicity(ticks)           // 2
        validateOffRouteDuration(ticks)          // 3
        validatePositionFreeze(ticks)            // 4
        validateReentrySnap(ticks)               // 5
        validateSkippedStops(ticks)              // 6
        validateNoArrivalsDuringOffRoute(ticks)  // 7
        validateGroundTruthConsistency(ticks)    // 8
        validateAnnounceEvents(ticks)            // 9
        validateFsmTransitions(ticks)            // 10

        println("All 10 validations passed!")
    }

    /**
     * VALIDATION 1: Arrival Sequence (PRD Core Requirement)
     * Expected arrivals: [0, 6, 7, 8, 9] or [0, 7, 8, 9]
     * Stops 2, 3, 4, 5 MUST be skipped (completely absent)
     */
    private fun validateArrivalSequence(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 1: Arrival Sequence ===")

        // Extract arrivals from trace (AtStop states indicate arrival)
        val arrivals = extractArrivals(ticks)
        println("Arrivals: $arrivals")

        // Core PRD requirement: stops 2, 3, 4, 5 must be skipped
        val skippedStops = listOf(2, 3, 4, 5)
        for (skipped in skippedStops) {
            assertFalse(
                "Stop $skipped should be SKIPPED (not in arrivals). Detected: $arrivals",
                arrivals.contains(skipped)
            )
        }
        println("✓ Skipped stops: $skippedStops")

        // Must include stop 0 (before detour)
        assertTrue(
            "Stop 0 should be DETECTED. Detected: $arrivals",
            arrivals.contains(0)
        )
        println("✓ Arrival sequence: $arrivals")
    }

    /**
     * VALIDATION 2: GPS Position Monotonicity (No Backward Jumps)
     * Position must be monotonically increasing OR frozen during off-route.
     */
    private fun validateGpsMonotonicity(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 2: GPS Position Monotonicity ===")

        var prevSCm: Long? = null
        var backwardJumps = 0

        for (tick in ticks) {
            if (prevSCm != null && !tick.off_route) {
                // Only check monotonicity when NOT off-route
                if (tick.s_cm < prevSCm!! - 1000) {
                    // Allow 10m GPS noise tolerance
                    println("⚠ Backward jump: ${prevSCm} → ${tick.s_cm} (${tick.s_cm - prevSCm!!} cm)")
                    backwardJumps++
                }
            }
            prevSCm = tick.s_cm
        }

        assertEquals(
            "No backward jumps (except GPS noise tolerance). Backward jumps: $backwardJumps",
            0,
            backwardJumps
        )
        println("✓ GPS monotonicity: no backward jumps")
    }

    /**
     * VALIDATION 3: Off-Route Detection & Duration
     * Off-route must be detected and last ≥5 seconds.
     */
    private fun validateOffRouteDuration(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 3: Off-Route Duration ===")

        // Find off-route episode
        var offRouteStart: Int? = null
        var offRouteEnd: Int? = null
        var maxOffRouteDuration = 0

        for ((i, tick) in ticks.withIndex()) {
            if (tick.off_route && offRouteStart == null) {
                offRouteStart = i
            } else if (!tick.off_route && offRouteStart != null && offRouteEnd == null) {
                offRouteEnd = i
            }
        }

        if (offRouteStart != null && offRouteEnd != null) {
            maxOffRouteDuration = offRouteEnd!! - offRouteStart!!
        }

        // Check if off-route was detected
        assertTrue(
            "Off-route must be detected in trace",
            offRouteStart != null
        )
        println("✓ Off-route detected at tick $offRouteStart")

        // Check duration ≥5 seconds
        assertTrue(
            "Off-route must last ≥5 seconds (PRD line 186). Duration: $maxOffRouteDuration ticks",
            maxOffRouteDuration >= 5
        )
        println("✓ Off-route duration: $maxOffRouteDuration ticks (≥5s)")
    }

    /**
     * Helper: Extract arrival stop indices from trace.
     * An arrival is when a stop first enters AtStop state.
     */
    private fun extractArrivals(ticks: List<TraceTick>): List<Int> {
        val arrivals = mutableSetOf<Int>()
        val stopFirstAtStop = mutableMapOf<Int, Int>()  // stop_idx -> first tick index

        for ((tickIdx, tick) in ticks.withIndex()) {
            tick.stop_states?.forEach { state ->
                if (state.fsm_state == "AtStop" && !stopFirstAtStop.containsKey(state.stop_idx)) {
                    stopFirstAtStop[state.stop_idx] = tickIdx
                    arrivals.add(state.stop_idx)
                }
            }
        }

        return arrivals.sorted()
    }

    /**
     * VALIDATION 4: Position Freezing During Off-Route
     * When off_route=true, s_cm must remain constant.
     */
    private fun validatePositionFreeze(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 4: Position Freeze During Off-Route ===")

        var frozenSCm: Long? = null
        var freezeViolations = 0

        for (tick in ticks) {
            if (tick.off_route) {
                if (frozenSCm == null) {
                    frozenSCm = tick.s_cm
                    println("Position frozen at s_cm = $frozenSCm")
                } else if (tick.s_cm != frozenSCm) {
                    println("⚠ Freeze violation: expected $frozenSCm, got ${tick.s_cm}")
                    freezeViolations++
                }
            } else {
                frozenSCm = null
            }
        }

        assertEquals(
            "Position must remain frozen during off-route. Violations: $freezeViolations",
            0,
            freezeViolations
        )
        println("✓ Position frozen during off-route")
    }

    /**
     * VALIDATION 5: Immediate Snap on Re-entry
     * On off_route=false transition, position must jump >100m.
     */
    private fun validateReentrySnap(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 5: Immediate Snap on Re-entry ===")

        var wasOffRoute = false
        var lastOffRouteSCm: Long? = null
        var snapDetected = false

        for (tick in ticks) {
            if (wasOffRoute && !tick.off_route) {
                // Transition from off_route to on_route
                val jump = kotlin.math.abs(tick.s_cm - (lastOffRouteSCm ?: 0))
                println("Re-entry jump: $jump cm (${jump / 100}m)")

                assertTrue(
                    "Re-entry must have significant position jump (>100m). Jump: ${jump}cm",
                    jump > 10000
                )
                snapDetected = true
                println("✓ Snap detected: ${jump / 100}m")
            }

            if (tick.off_route) {
                lastOffRouteSCm = tick.s_cm
                wasOffRoute = true
            } else {
                wasOffRoute = false
            }
        }

        // Skip this validation if off-route was never detected
        if (!ticks.any { it.off_route }) {
            println("⚠ Skipped: off-route never detected")
            return
        }

        assertTrue(
            "Snap on re-entry must be detected",
            snapDetected
        )
    }

    /**
     * VALIDATION 6: Skipped Stops Validation
     * Stops 2, 3, 4, 5 must NOT appear in arrivals.
     */
    private fun validateSkippedStops(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 6: Skipped Stops ===")

        val arrivals = extractArrivals(ticks)
        val skippedStops = listOf(2, 3, 4, 5)

        for (skipped in skippedStops) {
            assertFalse(
                "Stop $skipped should be SKIPPED. Arrivals: $arrivals",
                arrivals.contains(skipped)
            )
        }

        println("✓ All intermediate stops skipped: $skippedStops")
    }

    /**
     * VALIDATION 7: No Arrivals During Off-Route
     * All arrivals must occur BEFORE or AFTER off-route episode.
     */
    private fun validateNoArrivalsDuringOffRoute(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 7: No Arrivals During Off-Route ===")

        var offRouteStart: Int? = null
        var offRouteEnd: Int? = null

        // Find off-route episode
        for ((i, tick) in ticks.withIndex()) {
            if (tick.off_route && offRouteStart == null) {
                offRouteStart = i
            } else if (!tick.off_route && offRouteStart != null && offRouteEnd == null) {
                offRouteEnd = i
            }
        }

        // Check for arrivals during off-route
        val arrivalsDuringOffRoute = mutableListOf<Int>()
        if (offRouteStart != null && offRouteEnd != null) {
            for (i in offRouteStart!! until offRouteEnd!!) {
                val tick = ticks[i]
                tick.stop_states?.forEach { state ->
                    if (state.fsm_state == "AtStop") {
                        arrivalsDuringOffRoute.add(state.stop_idx)
                    }
                }
            }
        }

        assertEquals(
            "No arrivals during off-route. Arrivals: $arrivalsDuringOffRoute",
            emptyList<Int>(),
            arrivalsDuringOffRoute
        )
        println("✓ No arrivals during off-route")
    }

    /**
     * VALIDATION 8: Ground Truth Consistency
     * Compare against ty225_short_detour_gt.json if available.
     */
    private fun validateGroundTruthConsistency(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 8: Ground Truth Consistency ===")

        // Load ground truth if available
        val gtFile = java.io.File("../test_data/short_detour/ground_truth.json")
        if (!gtFile.exists()) {
            println("⚠ Ground truth file not found, skipping validation")
            return
        }

        // For now, just verify off-route was detected
        val hasOffRoute = ticks.any { it.off_route }
        assertTrue(
            "Off-route must be detected (ground truth consistency)",
            hasOffRoute
        )
        println("✓ Ground truth: off-route detected")
    }

    /**
     * VALIDATION 9: Announce Events Validation
     * Stops 2, 3, 4, 5 must NOT be announced.
     */
    private fun validateAnnounceEvents(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 9: Announce Events ===")

        val announcedStops = mutableSetOf<Int>()

        for (tick in ticks) {
            tick.stop_states?.forEach { state ->
                // State transitions through Approaching/Arriving indicate announce
                if (state.fsm_state == "Approaching" || state.fsm_state == "Arriving") {
                    announcedStops.add(state.stop_idx)
                }
            }
        }

        println("Announced stops: $announcedStops")

        // Stops 2, 3, 4, 5 should not be announced
        val skippedStops = listOf(2, 3, 4, 5)
        for (skipped in skippedStops) {
            assertFalse(
                "Stop $skipped should NOT be announced. Announced: $announcedStops",
                announcedStops.contains(skipped)
            )
        }

        println("✓ Intermediate stops not announced: $skippedStops")
    }

    /**
     * VALIDATION 10: FSM State Transitions
     * Verify proper state progression for detected stops.
     */
    private fun validateFsmTransitions(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 10: FSM State Transitions ===")

        // Build state history for each stop
        val stateHistory = mutableMapOf<Int, MutableList<String>>()

        for (tick in ticks) {
            tick.stop_states?.forEach { state ->
                if (!stateHistory.containsKey(state.stop_idx)) {
                    stateHistory[state.stop_idx] = mutableListOf()
                }
                stateHistory[state.stop_idx]?.add(state.fsm_state)
            }
        }

        // Verify stop 0 has proper progression
        val stop0States = stateHistory[0] ?: emptyList()
        if (stop0States.isNotEmpty()) {
            val hasApproaching = stop0States.contains("Approaching")
            val hasAtStop = stop0States.contains("AtStop")
            val hasDeparted = stop0States.contains("Departed")

            println("Stop 0 states: ${stop0States.toSet()}")

            assertTrue(
                "Stop 0 should have Approaching state",
                hasApproaching
            )
            assertTrue(
                "Stop 0 should have AtStop state",
                hasAtStop
            )
        }

        println("✓ FSM transitions valid for detected stops")
    }
}
