package com.busarrival.app.scenarios.common

import android.location.Location
import com.busarrival.app.data.pipeline.binary.RouteDataParser
import java.io.File

/**
 * Test data loader for scenario tests.
 */
object TestDataLoader {

    /**
     * Root test data directory.
     * Can be overridden via system property: -Dtest.data.root=/path/to/test_data
     */
    private val TEST_DATA_ROOT: String = System.getProperty("test.data.root")
        ?: findTestDataRoot().absolutePath

    private fun findTestDataRoot(): File {
        var current: File? = File(".").absoluteFile
        repeat(8) {
            val candidate = File(current, "test_data")
            if (candidate.isDirectory) {
                return candidate
            }
            current = current?.parentFile
        }
        error("Unable to locate test_data directory from ${File(".").absolutePath}. Set -Dtest.data.root explicitly.")
    }

    /**
     * Get full path to test data file.
     */
    private fun testPath(filename: String): String {
        return File(TEST_DATA_ROOT, filename).absolutePath
    }

    /**
     * Load route data binary file.
     */
    fun loadRouteData(scenario: String): com.busarrival.app.domain.model.RouteData {
        val path = testPath("ty225_${scenario}.bin")
        return RouteDataParser.loadFromFile(path)
    }

    /**
     * Load NMEA test data file.
     */
    fun loadNmea(scenario: String): List<Location> {
        val path = testPath("ty225_${scenario}_nmea.txt")
        val content = File(path).readText()
        return NmeaParser.parseFile(content)
    }

    /**
     * Load expected arrivals from ground truth JSON.
     */
    fun loadExpectedArrivals(scenario: String): List<Int> {
        val path = testPath("ty225_${scenario}_gt.json")
        val content = File(path).readText()

        // Parse JSON manually (minimal parsing)
        val stopIndices = mutableListOf<Int>()
        val lines = content.lines()
        for (line in lines) {
            val match = Regex("\"stop_idx\"\\s*:\\s*(\\d+)").find(line)
            if (match != null) {
                stopIndices.add(match.groupValues[1].toInt())
            }
        }
        return stopIndices
    }

    /**
     * Load raw text from a test data file under TEST_DATA_ROOT.
     */
    fun loadText(filename: String): String {
        val path = testPath(filename)
        return File(path).readText()
    }

    /**
     * Expected results for scenario validation.
     */
    data class ExpectedResults(
        val arrivals: List<Int>,
        val minArrivals: Int,
        val maxArrivals: Int
    ) {
        companion object {
            fun fromGroundTruth(scenario: String): ExpectedResults {
                val arrivals = loadExpectedArrivals(scenario)
                val count = arrivals.size
                return ExpectedResults(
                    arrivals = arrivals,
                    minArrivals = maxOf(0, count - 1),
                    maxArrivals = count + 1
                )
            }

            fun withBounds(min: Int, max: Int): ExpectedResults {
                return ExpectedResults(
                    arrivals = emptyList(),
                    minArrivals = min,
                    maxArrivals = max
                )
            }
        }
    }
}
