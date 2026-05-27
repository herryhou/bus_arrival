package com.busarrival.app.data.pipeline.localization.mapmatcher

import com.busarrival.app.domain.model.GridCell
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteNode
import com.busarrival.app.domain.model.SpatialGrid
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class MapMatcherTest {
    @Test
    fun firstFixRelaxesHeadingGateDuringGridSearch() {
        val routeData = routeWithOppositeHeadingGridCandidate()

        val coldStartMatch =
                MapMatcher.match(
                        gpsX = 10_000,
                        gpsY = 0,
                        gpsHeading = 0,
                        gpsSpeed = 500,
                        routeData = routeData,
                        lastIdx = 0,
                        isFirstFix = true
                )
        val warmMatch =
                MapMatcher.match(
                        gpsX = 10_000,
                        gpsY = 0,
                        gpsHeading = 0,
                        gpsSpeed = 500,
                        routeData = routeData,
                        lastIdx = 0,
                        isFirstFix = false
                )

        assertEquals(12, coldStartMatch.segIdx)
        assertEquals(0, coldStartMatch.dist2)
        assertNotEquals(12, warmMatch.segIdx)
    }

    private fun routeWithOppositeHeadingGridCandidate(): RouteData {
        val nodes =
                (0..12).map { idx ->
                    if (idx == 12) {
                        RouteNode(
                                xCm = 10_000,
                                yCm = 0,
                                cumDistCm = 10_000,
                                segLenMm = 10_000,
                                dxCm = 1_000,
                                dyCm = 0,
                                headingCdeg = 18_000
                        )
                    } else {
                        RouteNode(
                                xCm = idx * 100,
                                yCm = 100_000,
                                cumDistCm = idx * 100,
                                segLenMm = 10_000,
                                dxCm = 1_000,
                                dyCm = 0,
                                headingCdeg = 18_000
                        )
                    }
                }

        return RouteData(
                originLat = 20_000_000,
                originLon = 120_000_000,
                avgLat = 24_000_000,
                x0Cm = 0,
                y0Cm = 0,
                nodes = nodes,
                stops = emptyList(),
                grid =
                        SpatialGrid(
                                cellSizeCm = 100_000,
                                rows = 2,
                                cols = 1,
                                cells =
                                        listOf(
                                                GridCell(bitmask = 0UL, offsets = listOf(12)),
                                                GridCell(bitmask = 0UL, offsets = emptyList())
                                        )
                        )
        )
    }
}
