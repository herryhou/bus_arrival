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
        // TODO: Implement validations 4-10 in Tasks 8-9
        println("Validations 1-3 passed! Remaining: freeze, snap, skipped, no-offroute-arrivals, ground-truth, announce, fsm")
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
}
