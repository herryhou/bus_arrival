package com.busarrival.app.service

import android.location.Location
import com.busarrival.app.data.pipeline.binary.RouteDataParser
import com.busarrival.app.domain.model.GridCell
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
import com.busarrival.app.domain.model.SpatialGrid
import com.busarrival.app.domain.model.Stop
import com.busarrival.app.scenarios.common.NmeaParser
import com.busarrival.app.scenarios.common.TraceLoader
import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
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
            ticks.forEach { tick ->
                assertEquals(
                    "stop_states must contain only the current active stops at ${tick.gps.time_ms}",
                    tick.corridor.active_stops.sorted(),
                    tick.stop_states.map { it.stop_idx }.sorted()
                )
            }

            val firstTrace = Json.parseToJsonElement(
                traceFile.useLines { lines -> lines.first { it.isNotBlank() } }
            ).jsonObject
            assertEquals(
                setOf("gps", "kalman", "map_matching", "detection", "corridor", "stop_states"),
                firstTrace.keys
            )
            assertEquals("normal", firstTrace.getValue("detection").jsonObject.getValue("status").toString().trim('"'))

            val traceWithStopState = traceFile.useLines { lines ->
                lines
                    .filter { it.isNotBlank() }
                    .map { Json.parseToJsonElement(it).jsonObject }
                    .first { obj -> obj.getValue("stop_states").jsonArray.isNotEmpty() }
            }
            val firstStopState = traceWithStopState.getValue("stop_states").jsonArray.first().jsonObject

            // Check required fields exist (fields with default values may be omitted with encodeDefaults=false)
            val requiredKeys = setOf(
                "stop_idx",
                "gps_distance_cm",
                "progress_distance_cm",
                "fsm_state",
                "dwell_time_s",
                "probability",
                "features"
            )
            assertTrue(
                "Missing required keys: ${requiredKeys - firstStopState.keys}",
                firstStopState.keys.containsAll(requiredKeys)
            )

            assertTrue(
                "Detour trace should include off_route status at least once",
                ticks.any { it.detection.status == "off_route" }
            )

            val firstTickWithStopState = ticks.first { it.stop_states.isNotEmpty() }
            val firstParsedStopState = firstTickWithStopState.stop_states.first()
            assertEquals(0, firstParsedStopState.previous_probability)
            assertEquals(null, firstParsedStopState.previous_distance_cm)
            assertTrue(firstParsedStopState.fsm_state.isNotBlank())

            val consecutivePair = ticks
                .zipWithNext()
                .firstOrNull { (previous, current) ->
                    previous.stop_states.isNotEmpty() &&
                        current.stop_states.isNotEmpty() &&
                        previous.stop_states.first().stop_idx == current.stop_states.first().stop_idx
                }
            assertNotNull("Expected consecutive ticks for the same active stop", consecutivePair)
            val (previousTick, currentTick) = consecutivePair!!
            val previousStopState = previousTick.stop_states.first()
            val currentStopState = currentTick.stop_states.first()

            assertEquals(previousStopState.probability, currentStopState.previous_probability)
            assertEquals(previousStopState.progress_distance_cm, currentStopState.previous_distance_cm)
            assertFalse(
                "Previous distance should come from the prior tick, not the current one",
                currentStopState.previous_distance_cm == currentStopState.progress_distance_cm
            )
        } finally {
            pipeline.close()
            traceFile.delete()
        }
    }


    @Test
    fun process_gateClosedDoesNotAdvanceStopFsmState() {
        val traceFile = File.createTempFile("trace-v2-gate-closed", ".jsonl")
        val pipeline = DetectionPipeline()

        try {
            pipeline.initialize(straightSyntheticRoute(), traceFile = traceFile)

            listOf(
                syntheticLocation(xCm = 10_000, yCm = 6_000, timeMs = 1_000),
                syntheticLocation(xCm = 10_000, yCm = 6_000, timeMs = 2_000),
                syntheticLocation(xCm = 10_000, yCm = 6_000, timeMs = 3_000),
                syntheticLocation(xCm = 10_000, yCm = 6_000, timeMs = 4_000),
                syntheticLocation(xCm = 10_000, yCm = 0, timeMs = 5_000)
            ).forEach { pipeline.process(it) }
            pipeline.close()

            val ticks = TraceLoader.load(traceFile)
            assertEquals(5, ticks.size)

            ticks.take(4).forEachIndexed { idx, tick ->
                assertTrue(
                    "gate-closed tick[$idx] must not expose stop states",
                    tick.stop_states.isEmpty()
                )
                assertTrue(
                    "gate-closed tick[$idx] must not expose active stops",
                    tick.corridor.active_stops.isEmpty()
                )
            }

            val firstVisibleState = ticks[4].stop_states.single()
            assertEquals(0, firstVisibleState.stop_idx)
            assertEquals("Approaching", firstVisibleState.fsm_state)
            assertEquals(
                "FSM dwell time should start at first gate-open tick, not include hidden closed ticks",
                1,
                firstVisibleState.dwell_time_s
            )
            assertEquals(0, firstVisibleState.previous_probability)
            assertEquals(null, firstVisibleState.previous_distance_cm)
        } finally {
            pipeline.close()
            traceFile.delete()
        }
    }

    @Test
    fun process_updatesLastSegmentIndexAfterNormalMatch() {
        val traceFile = File.createTempFile("trace-v2-last-seg", ".jsonl")
        val pipeline = DetectionPipeline()

        try {
            pipeline.initialize(twoSegmentSyntheticRoute(), traceFile = traceFile)

            pipeline.process(syntheticLocation(xCm = 10_000, yCm = 0, timeMs = 1_000))
            pipeline.process(syntheticLocation(xCm = 40_000, yCm = 0, timeMs = 2_000))
            pipeline.close()

            val lastTick = TraceLoader.load(traceFile).last()
            assertEquals(1, lastTick.map_matching.segment_idx)
            assertEquals(1, pipeline.lastSegmentIndexForTest())
        } finally {
            pipeline.close()
            traceFile.delete()
        }
    }

    @Test
    fun process_tpF805TraceV2StopStatesMatchCurrentActiveStops() {
        val routeData = RouteDataParser.loadFromFile(testDataFile("tpF805_normal.bin").absolutePath)
        val locations = NmeaParser.parseFile(testDataFile("tpF805_normal_nmea.txt").readText())
        val traceFile = File.createTempFile("trace-v2-tpF805", ".jsonl")
        val pipeline = DetectionPipeline()

        try {
            pipeline.initialize(routeData, traceFile = traceFile)
            locations.forEach { pipeline.process(it) }
            pipeline.close()

            TraceLoader.load(traceFile).forEach { tick ->
                assertEquals(
                    "stop_states must contain only the current active stops at ${tick.gps.time_ms}",
                    tick.corridor.active_stops.sorted(),
                    tick.stop_states.map { it.stop_idx }.sorted()
                )
            }
        } finally {
            pipeline.close()
            traceFile.delete()
        }
    }


    private fun straightSyntheticRoute(): RouteData {
        return RouteData(
            originLat = 20_000_000,
            originLon = 120_000_000,
            avgLat = 24_000_000,
            x0Cm = 0,
            y0Cm = 0,
            nodes = listOf(
                RouteNode(
                    xCm = 0,
                    yCm = 0,
                    cumDistCm = 0,
                    segLenMm = 300_000,
                    dxCm = 30_000,
                    dyCm = 0,
                    headingCdeg = 0
                ),
                RouteNode(
                    xCm = 30_000,
                    yCm = 0,
                    cumDistCm = 30_000,
                    segLenMm = 0,
                    dxCm = 0,
                    dyCm = 0,
                    headingCdeg = 0
                )
            ),
            stops = listOf(
                Stop(
                    progressCm = 20_000,
                    corridorStartCm = 0,
                    corridorEndCm = 24_000
                )
            ),
            grid = SpatialGrid(
                cellSizeCm = 100_000,
                rows = 1,
                cols = 1,
                cells = listOf(GridCell(bitmask = 0UL, offsets = listOf(0)))
            )
        )
    }

    private fun twoSegmentSyntheticRoute(): RouteData {
        return RouteData(
            originLat = 20_000_000,
            originLon = 120_000_000,
            avgLat = 24_000_000,
            x0Cm = 0,
            y0Cm = 0,
            nodes = listOf(
                RouteNode(
                    xCm = 0,
                    yCm = 0,
                    cumDistCm = 0,
                    segLenMm = 300_000,
                    dxCm = 30_000,
                    dyCm = 0,
                    headingCdeg = 0
                ),
                RouteNode(
                    xCm = 30_000,
                    yCm = 0,
                    cumDistCm = 30_000,
                    segLenMm = 300_000,
                    dxCm = 30_000,
                    dyCm = 0,
                    headingCdeg = 0
                ),
                RouteNode(
                    xCm = 60_000,
                    yCm = 0,
                    cumDistCm = 60_000,
                    segLenMm = 0,
                    dxCm = 0,
                    dyCm = 0,
                    headingCdeg = 0
                )
            ),
            stops = emptyList(),
            grid = SpatialGrid(
                cellSizeCm = 100_000,
                rows = 1,
                cols = 1,
                cells = listOf(GridCell(bitmask = 0UL, offsets = listOf(0, 1)))
            )
        )
    }

    private fun syntheticLocation(xCm: Int, yCm: Int, timeMs: Long): Location {
        val (lat, lon) = gridToLatLon(xCm, yCm)
        return Location("synthetic").apply {
            latitude = lat
            longitude = lon
            time = timeMs
            speed = 5.0f
            bearing = 0.0f
            accuracy = 5.0f
        }
    }

    private fun gridToLatLon(xCm: Int, yCm: Int): Pair<Double, Double> {
        val earthRadiusCm = 637_100_000.0
        val avgLatRad = Math.toRadians(24.0)
        val lat = 20.0 + Math.toDegrees(yCm / earthRadiusCm)
        val lon = 120.0 + Math.toDegrees(xCm / (earthRadiusCm * kotlin.math.cos(avgLatRad)))
        return lat to lon
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

    private fun DetectionPipeline.lastSegmentIndexForTest(): Int {
        val stateField = DetectionPipeline::class.java.getDeclaredField("kalmanState")
        stateField.isAccessible = true
        val state = stateField.get(this)
        val segmentField = state.javaClass.getDeclaredField("lastSegIdx")
        segmentField.isAccessible = true
        return segmentField.getInt(state)
    }
}
