package com.busarrival.app.scenarios.common

import com.busarrival.app.service.TraceTick
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import java.io.File

/**
 * Helper to load grouped trace_v2.jsonl files for golden test validation.
 */
object TraceLoader {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = false  // Match TraceWriter configuration
    }

    /**
     * Load all ticks from a grouped v2 trace file.
     * @return List of TraceTick in order
     */
    fun load(file: File): List<TraceTick> {
        return file.readLines().mapNotNull { line ->
            if (line.isNotBlank()) {
                try {
                    json.decodeFromString<TraceTick>(line)
                } catch (e: Exception) {
                    println("Warning: Failed to parse trace line: $line")
                    null
                }
            } else null
        }
    }

    /**
     * Load trace from test data file.
     * @param scenarioName Scenario name (e.g., "short_detour")
     * @return List of TraceTick
     */
    fun loadFromTestData(scenarioName: String): List<TraceTick> {
        val file = File("../test_data/$scenarioName/trace_v2.jsonl")
        if (!file.exists()) {
            throw IllegalArgumentException("Trace file not found: ${file.absolutePath}")
        }
        return load(file)
    }
}
