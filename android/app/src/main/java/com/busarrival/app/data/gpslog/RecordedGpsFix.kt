package com.busarrival.app.data.gpslog

import android.location.Location
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import kotlin.math.abs
import kotlin.math.roundToLong

private const val SIMULATION_PROVIDER = "gps-log-simulation"

data class RecordedGpsFix(
    val timeMillis: Long,
    val latitude: Double,
    val longitude: Double,
    val accuracyM: Float?,
    val speedMps: Float?,
    val bearingDeg: Float?,
    val provider: String?,
    val isMock: Boolean
) {
    fun toLocation(): Location {
        return Location(provider ?: SIMULATION_PROVIDER).apply {
            time = timeMillis
            latitude = this@RecordedGpsFix.latitude
            longitude = this@RecordedGpsFix.longitude
            accuracyM?.let { accuracy = it }
            speedMps?.let { speed = it }
            bearingDeg?.let { bearing = it }
            if (isMock) markMockProvider()
        }
    }
}

object RecordedGpsLogParser {
    fun parseLines(lines: List<String>): Result<List<RecordedGpsFix>> {
        val fixes = lines.mapNotNull { line -> parseLine(line) }
        return if (fixes.isEmpty()) {
            Result.failure(IllegalArgumentException("GPS log contains no valid fixes"))
        } else {
            Result.success(fixes)
        }
    }

    private fun parseLine(line: String): RecordedGpsFix? {
        return runCatching {
            val obj = JsonParser.parseString(line).asJsonObject
            RecordedGpsFix(
                timeMillis = obj.requiredLong("t"),
                latitude = obj.requiredDouble("lat"),
                longitude = obj.requiredDouble("lon"),
                accuracyM = obj.optionalFloat("a"),
                speedMps = obj.optionalFloat("s"),
                bearingDeg = obj.optionalFloat("b"),
                provider = obj.optionalString("p"),
                isMock = obj.optionalBoolean("m") ?: false
            )
        }.getOrNull()
    }
}

object SupportedGpsPlaybackSpeeds {
    val values = listOf(0.5f, 1f, 2f, 4f)

    fun clamp(speed: Float): Float {
        if (speed <= 0f) return 1f
        return values.minBy { abs(it - speed) }
    }
}

object GpsSimulationTiming {
    fun delayMillis(previousTimeMillis: Long, nextTimeMillis: Long, playbackSpeed: Float): Long {
        val speed = SupportedGpsPlaybackSpeeds.clamp(playbackSpeed)
        val delta = (nextTimeMillis - previousTimeMillis).coerceAtLeast(0L)
        return (delta / speed).roundToLong()
    }
}

private fun JsonObject.requiredLong(name: String): Long = get(name).asLong

private fun JsonObject.requiredDouble(name: String): Double = get(name).asDouble

private fun JsonObject.optionalFloat(name: String): Float? = get(name)?.takeUnless { it.isJsonNull }?.asFloat

private fun JsonObject.optionalString(name: String): String? = get(name)?.takeUnless { it.isJsonNull }?.asString

private fun JsonObject.optionalBoolean(name: String): Boolean? = get(name)?.takeUnless { it.isJsonNull }?.asBoolean

private fun Location.markMockProvider() {
    runCatching {
        Location::class.java.getMethod("setMock", Boolean::class.javaPrimitiveType)
            .invoke(this, true)
    }.recoverCatching {
        Location::class.java.getDeclaredMethod("setIsFromMockProvider", Boolean::class.javaPrimitiveType)
            .apply { isAccessible = true }
            .invoke(this, true)
    }
}
