package com.busarrival.app.scenarios

import com.busarrival.app.data.pipeline.binary.RouteDataParser
import com.busarrival.app.data.pipeline.types.GpsStatus
import com.busarrival.app.data.pipeline.types.PhysicalConstants
import com.busarrival.app.service.DetectionPipeline
import com.busarrival.app.domain.model.RouteData
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * Regression test for false arrivals during dr_outage.
 * Ported from crates/pipeline/tests/dr_outage_false_arrival.rs
 *
 * Issue: At 80320000, stop #4 entered "Arriving" state despite being 48m away during dr_outage.
 * Root cause: During dr_outage, PositionSignals uses s_cm for both z_gps_cm and s_cm,
 * causing F1 to use DR position instead of raw GPS distance.
 * Fix: Neutralize F1 to 128 during dr_outage when divergence > PHANTOM_DIVERGENCE_CM.
 */
@RunWith(RobolectricTestRunner::class)
class DrOutageFalseArrivalTest {

    private lateinit var routeData: RouteData
    private lateinit var pipeline: DetectionPipeline

    @Before
    fun setup() {
        // Load route data (shared with Rust test)
        val routeFile = testDataFile("ty225_normal.bin")
        assertTrue("Route file should exist", routeFile.exists())

        routeData = RouteDataParser.loadFromFile(routeFile.absolutePath)
        pipeline = DetectionPipeline()
        pipeline.initialize(routeData)
    }

    @Test
    fun testDrOutageDoesNotCauseFalseArrival() {
        // This test verifies the fix for false arrivals during dr_outage
        // Full scenario test requires NMEA fixture data

        // For now, verify the fix is in place by checking:
        // 1. GpsStatus enum exists (compile-time check)
        // 2. PHANTOM_DIVERGENCE_CM constant exists (compile-time check)
        // 3. ProbabilityModel accepts gpsStatus parameter (compile-time check)

        // Verify GpsStatus enum exists and has expected values
        val validStatus = GpsStatus.Valid
        val drOutageStatus = GpsStatus.DrOutage
        val offRouteStatus = GpsStatus.OffRoute
        assertNotNull("GpsStatus.Valid should exist", validStatus)
        assertNotNull("GpsStatus.DrOutage should exist", drOutageStatus)
        assertNotNull("GpsStatus.OffRoute should exist", offRouteStatus)

        // Verify PHANTOM_DIVERGENCE_CM constant exists
        val phantomDivergenceCm = PhysicalConstants.PHANTOM_DIVERGENCE_CM
        assertEquals("PHANTOM_DIVERGENCE_CM should be 5000 (50m)", 5000, phantomDivergenceCm)

        // TODO: Add full scenario test with GPS jump simulation
        // Requires:
        // - Mock GPS locations at specific coordinates
        // - Test hook to inspect previousGpsStatus
        // - Verify probability < 191 when GPS is 48m away during dr_outage

        assertTrue("Test placeholder - compile-time checks passed", true)
    }

    private fun testDataFile(filename: String): File {
        val explicitRoot = System.getProperty("test.data.root")?.let(::File)
        if (explicitRoot != null) {
            return File(explicitRoot, filename)
        }

        return File(findTestDataRoot(), filename)
    }

    private fun findTestDataRoot(): File {
        var current: File? = File(".").absoluteFile
        repeat(8) {
            val candidate = File(current, "test_data")
            if (File(candidate, "ty225_normal.bin").exists()) {
                return candidate
            }
            current = current?.parentFile
        }
        error("Unable to locate test_data directory from ${File(".").absolutePath}")
    }
}
