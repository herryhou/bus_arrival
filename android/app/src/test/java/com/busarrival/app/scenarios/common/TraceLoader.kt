package com.busarrival.app.scenarios.common

import com.busarrival.app.service.TraceTick
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import java.io.File

/**
 * Helper to load trace.jsonl files for golden test validation.
 */
object TraceLoader {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    /**
     * Load all ticks from trace.jsonl file.
     * @return List of TraceTick in order
     */
    fun load(file: File): List<TraceTick> {
        return file.readLines().mapNotNull { line ->
            if (line.isNotBlank()) {
                try {
                    json.decodeFromString<TraceTick>(normalizeTraceLine(line))
                } catch (e: Exception) {
                    println("Warning: Failed to parse trace line: $line")
                    null
                }
            } else null
        }
    }

    private fun normalizeTraceLine(line: String): String {
        val obj = json.parseToJsonElement(line).jsonObject
        if (obj.containsKey("time_ms")) return line

        val legacyTime = obj["time"]?.jsonPrimitive?.long ?: return line
        val normalized: JsonObject = buildJsonObject {
            obj.forEach { (key, value) ->
                if (key != "time") put(key, value)
            }
            put("time_ms", JsonPrimitive(legacyTime * 1000))
        }
        return normalized.toString()
    }

    /**
     * Load trace from test data file.
     * @param scenarioName Scenario name (e.g., "short_detour")
     * @return List of TraceTick
     */
    fun loadFromTestData(scenarioName: String): List<TraceTick> {
        val file = File("../test_data/$scenarioName/trace.jsonl")
        if (!file.exists()) {
            throw IllegalArgumentException("Trace file not found: ${file.absolutePath}")
        }
        return load(file)
    }
}
