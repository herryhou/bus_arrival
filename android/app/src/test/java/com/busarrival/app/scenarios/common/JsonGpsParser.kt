package com.busarrival.app.scenarios.common

import android.location.Location
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.float

object JsonGpsParser {
    private val json = Json { ignoreUnknownKeys = true }

    fun parseJsonl(content: String): List<Location> {
        return content.lines()
            .filter { it.isNotBlank() }
            .mapNotNull { parseLine(it) }
    }

    private fun parseLine(line: String): Location? {
        val obj = json.parseToJsonElement(line).jsonObject

        val time = obj["t"]?.jsonPrimitive?.long ?: return null
        val lat = obj["lat"]?.jsonPrimitive?.float?.toDouble() ?: return null
        val lon = obj["lon"]?.jsonPrimitive?.float?.toDouble() ?: return null
        val accuracy = obj["a"]?.jsonPrimitive?.float
        val speed = obj["s"]?.jsonPrimitive?.float ?: 0f
        val bearing = obj["b"]?.jsonPrimitive?.float ?: 0f

        return Location("jsonl").apply {
            this.time = time
            this.latitude = lat
            this.longitude = lon
            if (accuracy != null) {
                this.accuracy = accuracy
            }
            this.speed = speed
            this.bearing = bearing
        }
    }
}
