package com.busarrival.app.data.gpslog

import com.busarrival.app.data.preferences.DetectionPreferences
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class GpsLogStorageManagerTest {
    private val context = RuntimeEnvironment.getApplication()
    private val logDir =
        File(context.getExternalFilesDir(null) ?: context.filesDir, "gps-logs").apply {
            deleteRecursively()
            mkdirs()
        }

    @Before
    fun setUp() {
        DetectionPreferences(context).apply {
            gpsLogTreeUri = null
            lastGpsLogReference = null
        }
        logDir.deleteRecursively()
        logDir.mkdirs()
    }

    @Test
    fun listLogsReturnsSortedMetadataForActiveStorage() = runTest {
        val older = File(logDir, "gps-log-older.jsonl").apply {
            writeText("""{"t":1}""")
            setLastModified(1_000L)
        }
        val newer = File(logDir, "gps-log-newer.jsonl").apply {
            writeText("""{"t":2}""")
            setLastModified(2_000L)
        }
        DetectionPreferences(context).lastGpsLogReference = newer.absolutePath

        val logs = GpsLogStorageManager.listLogs(context)

        assertEquals(listOf("gps-log-newer.jsonl", "gps-log-older.jsonl"), logs.map { it.filename })
        assertTrue(logs.first().isActive)
        assertFalse(logs.last().isActive)
        assertEquals(newer.length(), logs.first().sizeBytes)
        assertEquals(older.length(), logs.last().sizeBytes)
    }

    @Test
    fun loadLogReturnsOnlyNonBlankLines() = runTest {
        val log = File(logDir, "gps-log-read.jsonl").apply {
            writeText(
                """
                {"t":1}

                {"t":2}
                
                {"t":3}
                """.trimIndent()
            )
        }

        val lines = GpsLogStorageManager.loadLog(context, log.absolutePath)

        assertEquals(listOf("""{"t":1}""", """{"t":2}""", """{"t":3}"""), lines)
    }

    @Test
    fun deleteLogBlocksTheActiveReference() = runTest {
        val active = File(logDir, "gps-log-active.jsonl").apply {
            writeText("""{"t":1}""")
        }
        DetectionPreferences(context).lastGpsLogReference = active.absolutePath

        val deleted = GpsLogStorageManager.deleteLog(context, active.absolutePath)

        assertFalse(deleted)
        assertTrue(active.exists())
    }
}
