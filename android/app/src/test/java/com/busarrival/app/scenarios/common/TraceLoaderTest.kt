package com.busarrival.app.scenarios.common

import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

class TraceLoaderTest {
    @Test
    fun load_readsCanonicalTimeMs() {
        val file = writeTrace("""{"time_ms":80001000,"s_cm":100,"v_cms":10,"active_stops":[],"stop_states":[],"gps_jump":false,"recovery_idx":null,"heading_constraint_met":true,"divergence_cm":0,"variance_cm2":0,"off_route":false}""")

        try {
            assertEquals(80_001_000L, TraceLoader.load(file).single().time_ms)
        } finally {
            file.delete()
        }
    }

    @Test
    fun load_convertsLegacyTimeSecondsToTimeMs() {
        val file = writeTrace("""{"time":80001,"s_cm":100,"v_cms":10,"active_stops":[],"stop_states":[],"gps_jump":false,"recovery_idx":null,"heading_constraint_met":true,"divergence_cm":0,"variance_cm2":0,"off_route":false}""")

        try {
            assertEquals(80_001_000L, TraceLoader.load(file).single().time_ms)
        } finally {
            file.delete()
        }
    }

    private fun writeTrace(line: String): File {
        return File.createTempFile("trace-loader", ".jsonl").apply {
            writeText(line)
        }
    }
}
