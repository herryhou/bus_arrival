package com.busarrival.app.presentation.ui.detection.components

import kotlin.test.Test
import kotlin.test.assertEquals

class StatusPanelFormattingTest {
    @Test
    fun formatStopLabel_whenStopIsActive_usesOneBasedStopNumber() {
        assertEquals("Stop 3", formatStopLabel(2))
    }

    @Test
    fun formatStopLabel_whenNoStopIsActive_returnsNoActiveStop() {
        assertEquals("No active stop", formatStopLabel(-1))
    }

    @Test
    fun formatDistance_formatsCentimetersAsMeters() {
        assertEquals("123.5 m", formatDistance(12_345))
    }

    @Test
    fun formatSpeed_formatsCentimetersPerSecondAsKilometersPerHour() {
        assertEquals("18.0 km/h", formatSpeed(500))
    }

    @Test
    fun formatStateLabel_addsSpacesForCompactMachineState() {
        assertEquals("At stop", formatStateLabel("ATSTOP"))
        assertEquals("Trip complete", formatStateLabel("TRIPCOMPLETE"))
    }
}
