package com.busarrival.app.service

import com.busarrival.app.data.gpslog.GpsLogMetadata
import java.io.File
import java.util.zip.ZipFile
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class GpsLogArchiveTest {
    private val context = RuntimeEnvironment.getApplication()

    @Before
    fun setUp() {
        File(context.cacheDir, "gps-log-archives").deleteRecursively()
        File(context.filesDir, "gps-log-archive-a.jsonl").delete()
        File(context.filesDir, "gps-log-archive-b.jsonl").delete()
    }

    @Test
    fun archiveBundlesMultipleLogsIntoSingleZip() {
        val first = File(context.filesDir, "gps-log-archive-a.jsonl").apply {
            writeText(
                """
                {"t":1}
                {"t":2}
                """.trimIndent()
            )
        }
        val second = File(context.filesDir, "gps-log-archive-b.jsonl").apply {
            writeText("""{"t":3}""")
        }

        val zipFile =
            GpsLogArchive.createZip(
                context,
                listOf(
                    GpsLogMetadata(first.name, first.absolutePath, 2_000L, first.length(), false),
                    GpsLogMetadata(second.name, second.absolutePath, 1_000L, second.length(), false)
                )
            )

        assertTrue(zipFile.exists())

        ZipFile(zipFile).use { zip ->
            val entries = zip.entries().asSequence().map { it.name }.toList()
            assertEquals(listOf("gps-log-archive-a.jsonl", "gps-log-archive-b.jsonl"), entries)
            assertEquals(
                "{\"t\":1}\n{\"t\":2}\n",
                zip.getInputStream(zip.getEntry("gps-log-archive-a.jsonl")).bufferedReader().readText()
            )
            assertEquals(
                "{\"t\":3}\n",
                zip.getInputStream(zip.getEntry("gps-log-archive-b.jsonl")).bufferedReader().readText()
            )
        }
    }
}
