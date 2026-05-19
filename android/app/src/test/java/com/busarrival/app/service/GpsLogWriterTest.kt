package com.busarrival.app.service

import android.location.Location
import java.io.File
import java.nio.file.Files
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class GpsLogWriterTest {

    @Test
    fun openCreatesSessionFileAndWritesCompactJsonForPresentLocationFields() {
        val dir = tempDir()
        val writer = GpsLogWriter(FileGpsLogStore(dir), clock = { 1_764_543_905_000L })
        val location = Location("gps").apply {
            time = 1_700_000_000_123L
            elapsedRealtimeNanos = 987_654_321_000L
            latitude = 25.033
            longitude = 121.5654
            accuracy = 4.5f
            speed = 3.25f
            bearing = 182.75f
        }

        val status = writer.open(routeId = "ty225")
        writer.append(location)
        writer.close()

        assertTrue(status is GpsLogStatus.Active)
        val file = File(dir, "gps-log-ty225-20251130-230505.jsonl")
        assertTrue(file.exists(), "Expected session log file to be created")
        assertEquals(
            """{"t":1700000000123,"lat":25.033,"lon":121.5654,"a":4.5,"s":3.25,"b":182.75,"p":"gps"}""",
            file.readText().trim()
        )
    }

    @Test
    fun optionalLocationFieldsAreOmittedWhenMissing() {
        val dir = tempDir()
        val writer = GpsLogWriter(FileGpsLogStore(dir), clock = { 1_764_543_905_000L })
        val location = Location(null).apply {
            time = 10L
            elapsedRealtimeNanos = 20_000_000L
            latitude = 1.0
            longitude = 2.0
        }

        writer.open(routeId = null)
        writer.append(location)
        writer.close()

        val file = dir.listFiles()?.single() ?: error("Expected one log file")
        assertEquals("""{"t":10,"lat":1.0,"lon":2.0}""", file.readText().trim())
    }

    @Test
    fun appendOutsideOpenSessionDoesNotCreateOrWriteRows() {
        val dir = tempDir()
        val writer = GpsLogWriter(FileGpsLogStore(dir), clock = { 1_764_543_905_000L })
        val location = Location("gps").apply {
            time = 10L
            elapsedRealtimeNanos = 20_000_000L
            latitude = 1.0
            longitude = 2.0
        }

        writer.append(location)

        assertTrue(dir.listFiles().isNullOrEmpty())
    }

    @Test
    fun closePreventsLaterRowsAndFlushesExistingRows() {
        val dir = tempDir()
        val writer = GpsLogWriter(FileGpsLogStore(dir), clock = { 1_764_543_905_000L })
        val first = Location("gps").apply {
            time = 1L
            elapsedRealtimeNanos = 1_000_000L
            latitude = 1.0
            longitude = 2.0
        }
        val second = Location("gps").apply {
            time = 2L
            elapsedRealtimeNanos = 2_000_000L
            latitude = 3.0
            longitude = 4.0
        }

        writer.open(routeId = "route")
        writer.append(first)
        writer.close()
        writer.append(second)

        val lines = dir.listFiles()?.single()?.readLines() ?: emptyList()
        assertEquals(1, lines.size)
        assertTrue(lines.single().contains(""""t":1"""))
    }

    @Test
    fun openFailureDisablesLoggingWithoutThrowing() {
        val writer = GpsLogWriter(
            object : GpsLogStore {
                override fun create(routeId: String?, startedAtMillis: Long): GpsLogSession {
                    error("disk unavailable")
                }
            },
            clock = { 1L }
        )
        val location = Location("gps").apply {
            time = 1L
            elapsedRealtimeNanos = 1_000_000L
            latitude = 1.0
            longitude = 2.0
        }

        val status = writer.open(routeId = "route")
        writer.append(location)
        writer.close()

        assertTrue(status is GpsLogStatus.Disabled)
        assertFalse(writer.isActive)
    }

    @Test
    fun writeFailureDisablesRestOfSessionWithoutThrowing() {
        var writes = 0
        val writer = GpsLogWriter(
            object : GpsLogStore {
                override fun create(routeId: String?, startedAtMillis: Long): GpsLogSession {
                    return object : GpsLogSession {
                        override val description: String = "failing"
                        override fun append(line: String) {
                            writes++
                            error("write failed")
                        }
                        override fun close() = Unit
                    }
                }
            },
            clock = { 1L }
        )
        val location = Location("gps").apply {
            time = 1L
            elapsedRealtimeNanos = 1_000_000L
            latitude = 1.0
            longitude = 2.0
        }

        writer.open(routeId = "route")
        writer.append(location)
        writer.append(location)

        assertEquals(1, writes)
        assertFalse(writer.isActive)
    }

    @Test
    fun rowsPreserveCallbackOrder() {
        val dir = tempDir()
        val writer = GpsLogWriter(FileGpsLogStore(dir), clock = { 1_764_543_905_000L })

        writer.open(routeId = "route")
        for (time in listOf(3L, 1L, 2L)) {
            writer.append(
                Location("gps").apply {
                    this.time = time
                    elapsedRealtimeNanos = time * 1_000_000L
                    latitude = time.toDouble()
                    longitude = 0.0
                }
            )
        }
        writer.close()

        val loggedTimes = dir.listFiles()
            ?.single()
            ?.readLines()
            ?.map { Regex(""""t":(\d+)""").find(it)?.groupValues?.get(1)?.toLong() }

        assertEquals(listOf(3L, 1L, 2L), loggedTimes)
    }

    private fun tempDir(): File = Files.createTempDirectory("gps-log-writer").toFile()
}
