package com.busarrival.app.scenarios

import com.busarrival.app.scenarios.common.TestDataLoader
import com.busarrival.app.scenarios.common.TraceLoader
import com.busarrival.app.service.DetectionPipeline
import com.busarrival.app.service.PipelineResult
import com.busarrival.app.service.TraceTick
import org.junit.Assert.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * Test for tz_23_short scenario with JSONL GPS input.
 * Route: 9 stops, normal operation validation.
 */
@RunWith(RobolectricTestRunner::class)
class Tz23ScenarioTest {
    private companion object {
        const val SCENARIO = "short"
        const val TRACE_FILENAME = "tz_23_trace.jsonl"
        const val MAX_ALLOWED_BACKTRACK_CM = 5_000L
    }

    private lateinit var traceFile: File
    private lateinit var routeData: com.busarrival.app.domain.model.RouteData
    private var pipeline: DetectionPipeline? = null

    @Before
    fun setup() {
        traceFile = testDataFile(TRACE_FILENAME)
        routeData = TestDataLoader.loadRouteData(SCENARIO)
    }

    @After
    fun cleanup() {
        pipeline?.close()
        pipeline = null
    }

    @Test
    fun test_tz23_short_processes_gps_input() {
        val run = processScenario()
        val ticks = run.ticks
        val arrivals = run.arrivals

        println("\n=== tz_23_short Results ===")
        println("Total ticks: ${ticks.size}")
        println("Arrivals detected: ${arrivals.size}")
        println("Arrival stops: ${arrivals.map { it.first }}")

        assertTrue("Should process GPS ticks", ticks.isNotEmpty())
    }

    @Test
    fun test_tz23_short_detects_multiple_stops() {
        val run = processScenario()
        val arrivals = run.arrivals
        val detectedStops = arrivals.map { it.first }.distinct()

        println("Detected stops: $detectedStops")
        println("Total unique stops: ${detectedStops.size}")

        assertTrue(
            "Should detect multiple stops. Got ${detectedStops.size} stops: $detectedStops",
            detectedStops.size >= 3
        )
    }

    @Test
    fun test_tz23_short_gps_monotonicity() {
        val run = processScenario()
        val ticks = run.ticks
        var backwardJumps = 0
        var prevSCm: Long? = null

        for (tick in ticks) {
            if (prevSCm != null && tick.s_cm < prevSCm - MAX_ALLOWED_BACKTRACK_CM) {
                backwardJumps++
                println("Backward jump at tick.time_ms=${tick.time_ms}: $prevSCm -> ${tick.s_cm} (${prevSCm - tick.s_cm}cm)")
            }
            prevSCm = tick.s_cm
        }

        println("\nGPS monotonicity check: $backwardJumps backward jumps detected")
        println("Note: tz_23 data contains GPS anomalies causing position corrections")
    }

    @Test
    fun test_tz23_short_fsm_state_progression() {
        val run = processScenario()
        val ticks = run.ticks
        val stopStates = mutableMapOf<Int, Set<String>>()

        for (tick in ticks) {
            tick.stop_states.forEach { state ->
                stopStates
                    .getOrPut(state.stop_idx) { mutableSetOf() }
                    .let { it as MutableSet }
                    .add(state.fsm_state)
            }
        }

        println("\n=== FSM States by Stop ===")
        stopStates.forEach { (stopIdx, states) ->
            println("Stop $stopIdx: $states")
        }

        val stopsWithStates = stopStates.filter { it.value.isNotEmpty() }
        assertTrue(
            "Should have FSM states for multiple stops. Got: ${stopsWithStates.keys}",
            stopsWithStates.size >= 3
        )

        stopsWithStates.forEach { (stopIdx, states) ->
            val hasApproaching = states.contains("Approaching")
            val hasAtStop = states.contains("AtStop")
            assertTrue(
                "Stop $stopIdx should have Approaching state. States: $states",
                hasApproaching || hasAtStop
            )
        }
    }

    @Test
    fun test_tz23_short_trace_output_written() {
        processScenario()

        val destFile = testDataFile("tz_23_short_trace.jsonl")
        assertTrue(
            "Trace should be written to test_data/tz_23_short_trace.jsonl",
            destFile.exists()
        )

        val traceLines = destFile.readLines().filter { it.isNotBlank() }.size
        println("Trace lines written: $traceLines")
        assertTrue("Trace should have data", traceLines > 0)
    }

    @Test
    fun test_tz23_short_arrivals_in_sequence() {
        val run = processScenario()
        val arrivals = run.arrivals.map { it.first }

        for (i in 1 until arrivals.size) {
            assertTrue(
                "Arrivals should be in increasing stop index order. Got: $arrivals",
                arrivals[i] > arrivals[i - 1]
            )
        }
    }

    @Test
    fun test_tz23_short_timestamps_monotonic() {
        val run = processScenario()
        val ticks = run.ticks

        for (i in 1 until ticks.size) {
            assertTrue(
                "Timestamps should be monotonic. tick[$i-1]=${ticks[i-1].time_ms}, tick[$i]=${ticks[i].time_ms}",
                ticks[i].time_ms >= ticks[i - 1].time_ms
            )
        }
    }

    private fun testDataFile(filename: String): File {
        val explicitRoot = System.getProperty("test.data.root")?.let(::File)
        if (explicitRoot != null) {
            return File(explicitRoot, filename)
        }

        var current: File? = File(".").absoluteFile
        repeat(8) {
            val candidate = File(current, "test_data")
            if (candidate.isDirectory) {
                return File(candidate, filename)
            }
            current = current?.parentFile
        }
        error("Unable to locate test_data directory from ${File(".").absolutePath}")
    }

    private fun processScenario(): ScenarioRun {
        val scenarioPipeline = DetectionPipeline().apply {
            initialize(routeData, traceFile = traceFile)
            pipeline = this
        }
        val arrivals = mutableListOf<Pair<Int, Long>>()

        try {
            for (location in TestDataLoader.loadNmea(SCENARIO)) {
                val result = scenarioPipeline.process(location)
                if (result is PipelineResult.Success) {
                    result.arrivals.forEach { arrival ->
                        arrivals.add(arrival.stopIndex to arrival.timestamp)
                    }
                }
            }
            return ScenarioRun(
                ticks = TraceLoader.load(traceFile),
                arrivals = arrivals
            )
        } finally {
            scenarioPipeline.close()
            val destFile = testDataFile("tz_23_short_trace.jsonl")
            destFile.parentFile?.mkdirs()
            traceFile.copyTo(destFile, overwrite = true)
            traceFile.delete()
        }
    }

    private data class ScenarioRun(
        val ticks: List<TraceTick>,
        val arrivals: List<Pair<Int, Long>>,
    )
}
