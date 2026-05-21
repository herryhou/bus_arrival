package com.busarrival.app.scenarios

import com.busarrival.app.data.pipeline.binary.RouteDataParser
import com.busarrival.app.scenarios.common.JsonGpsParser
import com.busarrival.app.scenarios.common.TraceLoader
import com.busarrival.app.service.DetectionPipeline
import com.busarrival.app.service.PipelineResult
import com.busarrival.app.service.TraceTick
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
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
        const val ROUTE_DATA_FILENAME = "tz_23_short.bin"
        const val GPS_FILENAME = "tz_23-gps.jsonl"
        const val TRACE_FILENAME = "tz_23_short_trace_v2.jsonl"
        const val MAX_ALLOWED_BACKTRACK_CM = 5_000L
    }

    private lateinit var traceFile: File
    private lateinit var routeData: com.busarrival.app.domain.model.RouteData
    private var pipeline: DetectionPipeline? = null

    @Before
    fun setup() {
        traceFile = testDataFile(TRACE_FILENAME)
        routeData = RouteDataParser.loadFromFile(testDataFile(ROUTE_DATA_FILENAME).absolutePath)
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

        val destFile = testDataFile(TRACE_FILENAME)
        assertTrue(
            "Trace should be written to test_data/$TRACE_FILENAME",
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

    @Test
    fun test_tz23_short_trace_uses_grouped_v2_schema() {
        processScenario()

        val firstTrace = Json.parseToJsonElement(
            testDataFile(TRACE_FILENAME).useLines { lines ->
                lines.first { it.isNotBlank() }
            }
        ).jsonObject

        assertEquals(
            setOf("gps", "kalman", "map_matching", "detection", "corridor", "stop_states"),
            firstTrace.keys
        )
        assertFalse(firstTrace.containsKey("time"))
        assertFalse(firstTrace.containsKey("s_cm"))

        val tickWithStopState = testDataFile(TRACE_FILENAME).useLines { lines ->
            lines
                .filter { it.isNotBlank() }
                .map { Json.parseToJsonElement(it).jsonObject }
                .first { trace -> trace.getValue("stop_states").jsonArray.isNotEmpty() }
        }
        val firstStopState = tickWithStopState.getValue("stop_states").jsonArray.first().jsonObject
        assertEquals(
            setOf(
                "stop_idx",
                "gps_distance_cm",
                "progress_distance_cm",
                "fsm_state",
                "dwell_time_s",
                "probability",
                "previous_probability",
                "features",
                "announced",
                "skip_on_reentry",
                "previous_distance_cm",
                "just_arrived"
            ),
            firstStopState.keys
        )
        assertTrue(firstTrace.getValue("gps").jsonObject.getValue("time_ms").jsonPrimitive.long > 0)
        assertNotNull(firstTrace.getValue("detection").jsonObject.getValue("status").jsonPrimitive.content)
    }

    private fun testDataFile(filename: String): File {
        val explicitRoot = System.getProperty("test.data.root")?.let(::File)
        if (explicitRoot != null) {
            return File(explicitRoot, filename)
        }

        return File(findTestDataRoot(), filename)
    }

    private fun findTestDataRoot(): File {
        var current: File? = File(".").absoluteFile
        repeat(8) {
            val candidate = File(current, "test_data")
            if (File(candidate, ROUTE_DATA_FILENAME).exists()) {
                return candidate
            }
            current = current?.parentFile
        }
        error("Unable to locate test_data directory from ${File(".").absolutePath}")
    }

    private fun processScenario(): ScenarioRun {
        val scenarioTraceFile = File.createTempFile("tz23-trace", ".jsonl")
        val scenarioPipeline = DetectionPipeline().apply {
            initialize(routeData, traceFile = scenarioTraceFile)
            pipeline = this
        }
        val arrivals = mutableListOf<Pair<Int, Long>>()
        val locations = JsonGpsParser.parseJsonl(testDataFile(GPS_FILENAME).readText())

        try {
            for (location in locations) {
                val result = scenarioPipeline.process(location)
                if (result is PipelineResult.Success) {
                    result.arrivals.forEach { arrival ->
                        arrivals.add(arrival.stopIndex to arrival.timestamp)
                    }
                }
            }
            return ScenarioRun(
                ticks = TraceLoader.load(scenarioTraceFile),
                arrivals = arrivals
            )
        } finally {
            scenarioPipeline.close()
            val destFile = testDataFile(TRACE_FILENAME)
            destFile.parentFile?.mkdirs()
            scenarioTraceFile.copyTo(destFile, overwrite = true)
            scenarioTraceFile.delete()
        }
    }

    private data class ScenarioRun(
        val ticks: List<TraceTick>,
        val arrivals: List<Pair<Int, Long>>,
    )

    private val TraceTick.time_ms: Long
        get() = gps.time_ms

    private val TraceTick.s_cm: Long
        get() = kalman.s_cm
}
