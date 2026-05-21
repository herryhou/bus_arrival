package com.busarrival.app.scenarios.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class TraceLoaderTest {
    @Test
    fun load_readsGroupedV2TraceTick() {
        val file = writeTrace(
            """
            {"gps":{"time_ms":80001000,"lat":25.0,"lon":121.0,"heading_cdeg":1234,"hdop":0.9,"accuracy_cm":450},"kalman":{"s_cm":100,"v_cms":10,"variance_cm2":25,"divergence_cm":3},"map_matching":{"segment_idx":7,"heading_constraint_met":true},"detection":{"status":"normal","off_route":false,"gps_jump":false,"recovery_idx":null},"corridor":{"active_stops":[2],"corridor_start_cm":80,"corridor_end_cm":140,"next_stop":[3,77]},"stop_states":[{"stop_idx":2,"gps_distance_cm":-20,"progress_distance_cm":-15,"fsm_state":"Approaching","dwell_time_s":4,"probability":80,"previous_probability":70,"features":{"p1":1,"p2":2,"p3":3,"p4":4},"announced":true,"skip_on_reentry":false,"previous_distance_cm":-30,"just_arrived":false}]}
            """.trimIndent()
        )

        try {
            val tick = TraceLoader.load(file).single()

            assertEquals(80_001_000L, tick.gps.time_ms)
            assertEquals(100L, tick.kalman.s_cm)
            assertEquals(10, tick.kalman.v_cms)
            assertEquals(25, tick.kalman.variance_cm2)
            assertEquals(3, tick.kalman.divergence_cm)
            assertEquals(7, tick.map_matching.segment_idx)
            assertTrue(tick.map_matching.heading_constraint_met)
            assertEquals("normal", tick.detection.status)
            assertFalse(tick.detection.off_route)
            assertFalse(tick.detection.gps_jump)
            assertEquals(listOf(2), tick.corridor.active_stops)
            assertEquals(80, tick.corridor.corridor_start_cm)
            assertEquals(140, tick.corridor.corridor_end_cm)
            assertEquals(listOf(3, 77), tick.corridor.next_stop)
            assertEquals(1, tick.stop_states.size)
            assertEquals(70, tick.stop_states.single().previous_probability)
            assertTrue(tick.stop_states.single().announced)
            assertFalse(tick.stop_states.single().skip_on_reentry)
            assertEquals(-30, tick.stop_states.single().previous_distance_cm)
        } finally {
            file.delete()
        }
    }

    @Test
    fun load_rejectsLegacyFlatTraceTick() {
        val file = writeTrace(
            """{"time_ms":80001000,"lat":25.0,"lon":121.0,"s_cm":100,"v_cms":10,"active_stops":[],"stop_states":[],"gps_jump":false,"recovery_idx":null,"heading_constraint_met":true,"divergence_cm":0,"variance_cm2":0,"off_route":false}"""
        )

        try {
            assertTrue(TraceLoader.load(file).isEmpty())
        } finally {
            file.delete()
        }
    }

    @Test
    fun loadFromTestData_readsTraceV2File() {
        val scenarioName = "trace-loader-test-${System.nanoTime()}"
        val scenarioDir = File("../test_data/$scenarioName")
        val traceFile = File(scenarioDir, "trace_v2.jsonl")

        scenarioDir.mkdirs()
        traceFile.writeText(
            """{"gps":{"time_ms":42},"kalman":{"s_cm":100},"map_matching":{"heading_constraint_met":false},"detection":{"status":"normal","off_route":false},"corridor":{"active_stops":[]},"stop_states":[]}"""
        )

        try {
            val tick = TraceLoader.loadFromTestData(scenarioName).single()
            assertEquals(42L, tick.gps.time_ms)
            assertEquals(100L, tick.kalman.s_cm)
        } finally {
            scenarioDir.deleteRecursively()
        }
    }

    private fun writeTrace(line: String): File {
        return File.createTempFile("trace-loader", ".jsonl").apply {
            writeText(line)
        }
    }
}
