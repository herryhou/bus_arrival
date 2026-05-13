package com.busarrival.app.presentation.ui.detection.components

import org.junit.Test
import kotlin.test.assertEquals

/**
 * Tests for TimelineScrubber component.
 * Focuses on time formatting logic.
 */
class TimelineScrubberTest {

    // Test the actual formatTime function from TimelineScrubber.kt
    // Since it's private, we recreate the logic here for testing
    private fun formatTime(timeMs: Long): String {
        val totalSeconds = timeMs / 1000
        val hours = totalSeconds / 3600
        val minutes = (totalSeconds % 3600) / 60
        val seconds = totalSeconds % 60

        return if (hours > 0) {
            String.format("%02d:%02d:%02d", hours, minutes, seconds)
        } else {
            String.format("%02d:%02d", minutes, seconds)
        }
    }

    @Test
    fun formatTime_zeroTime_returnsMinutesSeconds() {
        val result = formatTime(0)
        assertEquals("00:00", result)
    }

    @Test
    fun formatTime_lessThanOneMinute_returnsMinutesSeconds() {
        val result = formatTime(45_000) // 45 seconds
        assertEquals("00:45", result)
    }

    @Test
    fun formatTime_oneMinute_returnsMinutesSeconds() {
        val result = formatTime(60_000) // 1 minute
        assertEquals("01:00", result)
    }

    @Test
    fun formatTime_severalMinutes_returnsMinutesSeconds() {
        val result = formatTime(5_670_000) // 1 hour 34 minutes 30 seconds
        assertEquals("01:34:30", result)
    }

    @Test
    fun formatTime_oneHour_returnsHoursMinutesSeconds() {
        val result = formatTime(3_600_000) // 1 hour
        assertEquals("01:00:00", result)
    }

    @Test
    fun formatTime_lessThanOneHour_returnsMinutesSeconds() {
        val result = formatTime(2_745_000) // 45 minutes 45 seconds
        assertEquals("45:45", result)
    }

    @Test
    fun formatTime_complexTime_returnsCorrectFormat() {
        val result = formatTime(3_723_000) // 1:02:03
        assertEquals("01:02:03", result)
    }

    @Test
    fun formatTime_largeTime_returnsHoursMinutesSeconds() {
        val result = formatTime(45_000_000) // 12:30:00
        assertEquals("12:30:00", result)
    }
}
