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
        // TODO: Implement validations in Tasks 7-9
        println("Trace loaded with ${ticks.size} ticks - validations to be implemented")
        println("Validations: arrival, monotonicity, off-route, freeze, snap, skipped, no-offroute-arrivals, ground-truth, announce, fsm")
    }

    // ... validation functions will be added in next tasks
}
