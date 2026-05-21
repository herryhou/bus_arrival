package com.busarrival.app.service

import com.busarrival.app.data.pipeline.binary.RouteDataParser
import com.busarrival.app.scenarios.common.NmeaParser
import com.busarrival.app.scenarios.common.TraceLoader
import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class DetectionPipelineTraceV2Test {
    @Test
    fun process_writesGroupedTraceV2WithExpandedStopStateFields() {
        val routeData = RouteDataParser.loadFromFile(testDataFile("ty225_short_detour.bin").absolutePath)
        val locations = NmeaParser.parseFile(testDataFile("ty225_short_detour_nmea.txt").readText())
        val traceFile = File.createTempFile("trace-v2-pipeline", ".jsonl")
        val pipeline = DetectionPipeline()

        try {
            pipeline.initialize(routeData, traceFile = traceFile)
            locations.forEach { pipeline.process(it) }
            pipeline.close()

            val ticks = TraceLoader.load(traceFile)
            assertTrue("Expected grouped trace ticks to be parsed", ticks.isNotEmpty())

            val firstTrace = Json.parseToJsonElement(
                traceFile.useLines { lines -> lines.first { it.isNotBlank() } }
            ).jsonObject
            assertEquals(
                setOf("gps", "kalman", "map_matching", "detection", "corridor", "stop_states"),
                firstTrace.keys
            )

            val traceWithStopState = traceFile.useLines { lines ->
                lines
                    .filter { it.isNotBlank() }
                    .map { Json.parseToJsonElement(it).jsonObject }
                    .first { obj -> obj.getValue("stop_states").jsonArray.isNotEmpty() }
            }
            val firstStopState = traceWithStopState.getValue("stop_states").jsonArray.first().jsonObject

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

            val parsedStopState = ticks.first { it.stop_states.isNotEmpty() }.stop_states.first()
            assertTrue(parsedStopState.previous_probability >= 0)
            assertTrue(parsedStopState.fsm_state.isNotBlank())
        } finally {
            pipeline.close()
            traceFile.delete()
        }
    }

    private fun testDataFile(filename: String): File {
        var current: File? = File(".").absoluteFile
        repeat(8) {
            val candidate = File(current, "test_data")
            val target = File(candidate, filename)
            if (target.isFile) {
                return target
            }
            current = current?.parentFile
        }
        error("Unable to locate test_data/$filename from ${File(".").absolutePath}")
    }
}
