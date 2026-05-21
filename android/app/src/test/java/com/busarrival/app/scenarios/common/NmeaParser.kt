package com.busarrival.app.scenarios.common

import android.location.Location

/**
 * NMEA parser for scenario tests.
 * Parses GPRMC/GPGGA sentences to create Location objects.
 */
object NmeaParser {

    /**
     * Parse NMEA sentence to Location.
     * Returns null if sentence is not GPRMC/GPGGA.
     */
    fun parse(sentence: String): Location? {
        return when {
            sentence.startsWith("\$GPRMC") -> parseGPRMC(sentence)
            sentence.startsWith("\$GPGGA") -> parseGPGGA(sentence)
            else -> null
        }
    }

    /**
     * Parse GPRMC sentence.
     */
    private fun parseGPRMC(sentence: String): Location? {
        val parts = sentence.split(",")
        if (parts.size < 12) return null

        val time = parts[1]
        val status = parts[2]
        if (status != "A") return null  // Not valid

        val lat = parseLatLon(parts[3], parts[4])
        val lon = parseLatLon(parts[5], parts[6])
        val speedKnots = parts[7].toFloatOrNull() ?: 0f
        val heading = parts[8].toFloatOrNull() ?: 0f

        return createLocation(lat, lon, speedKnots, heading).apply {
            // Parse time if available
            if (time.length >= 6) {
                val hour = time.substring(0, 2).toIntOrNull() ?: 0
                val min = time.substring(2, 4).toIntOrNull() ?: 0
                val sec = time.substring(4, 6).toIntOrNull() ?: 0
                this.time = hour * 3600000L + min * 60000L + sec * 1000L
            }
        }
    }

    /**
     * Parse GPGGA sentence.
     */
    private fun parseGPGGA(sentence: String): Location? {
        val parts = sentence.split(",")
        if (parts.size < 10) return null

        val lat = parseLatLon(parts[2], parts[3])
        val lon = parseLatLon(parts[4], parts[5])
        val fixQuality = parts[6].toIntOrNull() ?: 0
        if (fixQuality == 0) return null  // No fix

        return createLocation(lat, lon, 0f, 0f).apply {
            // GPGGA has time in parts[1]
            val time = parts[1]
            if (time.length >= 6) {
                val hour = time.substring(0, 2).toIntOrNull() ?: 0
                val min = time.substring(2, 4).toIntOrNull() ?: 0
                val sec = time.substring(4, 6).toIntOrNull() ?: 0
                this.time = hour * 3600000L + min * 60000L + sec * 1000L
            }
        }
    }

    /**
     * Parse latitude/longitude from NMEA format.
     * Format: DDMM.MMMMM,N/S or DDDMM.MMMMM,E/W
     */
    private fun parseLatLon(value: String, dir: String): Double {
        if (value.length < 6) return 0.0

        val isLon = dir.startsWith("E") || dir.startsWith("W")
        val degLen = if (isLon) 3 else 2

        val degrees = value.substring(0, degLen).toDoubleOrNull() ?: 0.0
        val minutes = value.substring(degLen).toDoubleOrNull() ?: 0.0

        var result = degrees + minutes / 60.0

        // Apply sign based on direction
        if (dir == "S" || dir == "W") {
            result = -result
        }

        return result
    }

    /**
     * Create Location from parsed values.
     */
    private fun createLocation(lat: Double, lon: Double, speedKnots: Float, heading: Float): Location {
        return Location("nmea").apply {
            latitude = lat
            longitude = lon
            speed = speedKnots * 0.514444f  // Knots to m/s
            bearing = heading
        }
    }

    /**
     * Parse NMEA file to sequence of Locations.
     * Only returns GPRMC sentences (avoid duplicates).
     */
    fun parseFile(content: String): List<Location> {
        val locations = mutableListOf<Location>()
        var pendingRmc: Location? = null

        content.lineSequence().forEach { line ->
            when {
                line.startsWith("\$GPRMC") -> {
                    pendingRmc?.let { locations.add(it) }
                    pendingRmc = parseGPRMC(line)
                }
                line.startsWith("\$GPGGA") -> {
                    val accuracyM = parseGPGGAAccuracy(line)
                    if (accuracyM != null) {
                        pendingRmc?.accuracy = accuracyM * 10f
                    }
                    pendingRmc?.let {
                        locations.add(it)
                        pendingRmc = null
                    }
                }
            }
        }

        pendingRmc?.let { locations.add(it) }

        return locations.filter { it.speed > 0 || it.bearing > 0 }
    }

    /**
     * Parse GPGGA HDOP for a paired GPRMC fix.
     */
    private fun parseGPGGAAccuracy(sentence: String): Float? {
        val parts = sentence.split(",")
        if (parts.size < 9) return null
        return parts[8].toFloatOrNull()
    }
}
