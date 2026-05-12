package com.busarrival.app.data.trace

import android.content.Context
import com.busarrival.app.service.PipelineEvent
import com.google.gson.Gson
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Before
import org.junit.Test
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class TraceStorageManagerTest {

    private lateinit var mockContext: Context
    private lateinit var mockFilesDir: File
    private lateinit var testTracesDir: File
    private val gson = Gson()

    @Before
    fun setup() {
        mockContext = mockk()
        mockFilesDir = File(System.getProperty("java.io.tmpdir"), "app_test_${System.currentTimeMillis()}")
        mockFilesDir.mkdirs()
        testTracesDir = File(mockFilesDir, "traces")
        testTracesDir.mkdirs()

        every { mockContext.filesDir } returns mockFilesDir

        TraceStorageManager.init(mockContext, gson)
    }

    @After
    fun tearDown() {
        mockFilesDir.deleteRecursively()
    }

    @Test
    fun `init stores context and gson`() {
        // Verify initialization works
        assertTrue(testTracesDir.exists())
    }

    @Test
    fun `saveTrace creates JSONL file`() = runTest {
        val events = listOf(
            PipelineEvent.PositionUpdate(sCm = 1000, vCms = 500, mode = "Normal"),
            PipelineEvent.Arrival(stopIndex = 0, probability = 80),
            PipelineEvent.Departure(stopIndex = 0, dwellTimeS = 30)
        )

        val filename = "test_trace.jsonl"
        val result = TraceStorageManager.saveTrace(events, filename)

        assertTrue(result.isSuccess)

        val file = File(testTracesDir, filename)
        assertTrue(file.exists())

        val content = file.readText()
        val lines = content.lines()
        assertEquals(3, lines.size)

        // Verify JSON structure
        val loaded = TraceStorageManager.loadTrace(filename)
        assertEquals(3, loaded.size)
        assertTrue(loaded[0] is PipelineEvent.PositionUpdate)
        assertTrue(loaded[1] is PipelineEvent.Arrival)
        assertTrue(loaded[2] is PipelineEvent.Departure)
    }

    @Test
    fun `loadTrace reads and parses JSONL file`() = runTest {
        val events = listOf(
            PipelineEvent.PositionUpdate(sCm = 1000, vCms = 500, mode = "Normal"),
            PipelineEvent.Arrival(stopIndex = 0, probability = 80),
            PipelineEvent.Departure(stopIndex = 0, dwellTimeS = 30)
        )

        val filename = "test_trace_load.jsonl"
        TraceStorageManager.saveTrace(events, filename)

        val loaded = TraceStorageManager.loadTrace(filename)

        assertEquals(3, loaded.size)
        assertTrue(loaded[0] is PipelineEvent.PositionUpdate)
        assertTrue(loaded[1] is PipelineEvent.Arrival)
        assertTrue(loaded[2] is PipelineEvent.Departure)
    }

    @Test
    fun `loadTrace returns empty list for non-existent file`() = runTest {
        val loaded = TraceStorageManager.loadTrace("nonexistent.jsonl")
        assertTrue(loaded.isEmpty())
    }

    @Test
    fun `listTraces returns metadata for all traces`() = runTest {
        val events1 = listOf(
            PipelineEvent.PositionUpdate(sCm = 1000, vCms = 500, mode = "Normal")
        )
        val events2 = listOf(
            PipelineEvent.PositionUpdate(sCm = 2000, vCms = 600, mode = "Normal"),
            PipelineEvent.PositionUpdate(sCm = 2100, vCms = 550, mode = "Normal")
        )

        TraceStorageManager.saveTrace(events1, "trace1.jsonl")
        TraceStorageManager.saveTrace(events2, "trace2.jsonl")

        val traces = TraceStorageManager.listTraces()

        assertEquals(2, traces.size)
        assertTrue(traces.any { it.filename == "trace1.jsonl" })
        assertTrue(traces.any { it.filename == "trace2.jsonl" })
    }

    @Test
    fun `listTraces returns empty list when no traces exist`() = runTest {
        val traces = TraceStorageManager.listTraces()
        assertTrue(traces.isEmpty())
    }

    @Test
    fun `listTraces includes record count in metadata`() = runTest {
        val events = listOf(
            PipelineEvent.PositionUpdate(sCm = 1000, vCms = 500, mode = "Normal"),
            PipelineEvent.Arrival(stopIndex = 0, probability = 80),
            PipelineEvent.Departure(stopIndex = 0, dwellTimeS = 30)
        )

        TraceStorageManager.saveTrace(events, "test_metadata.jsonl")

        val traces = TraceStorageManager.listTraces()
        val trace = traces.find { it.filename == "test_metadata.jsonl" }

        assertTrue(trace != null)
        assertEquals(3, trace!!.recordCount)
    }

    @Test
    fun `deleteTrace removes trace file`() = runTest {
        val events = listOf(
            PipelineEvent.PositionUpdate(sCm = 1000, vCms = 500, mode = "Normal")
        )

        val filename = "test_delete.jsonl"
        TraceStorageManager.saveTrace(events, filename)

        var file = File(testTracesDir, filename)
        assertTrue(file.exists())

        TraceStorageManager.deleteTrace(filename)

        file = File(testTracesDir, filename)
        assertFalse(file.exists())
    }

    @Test
    fun `deleteTrace succeeds for non-existent file`() = runTest {
        val result = TraceStorageManager.deleteTrace("nonexistent.jsonl")
        assertTrue(result.isSuccess)
    }

    @Test
    fun `saveTrace overwrites existing file`() = runTest {
        val events1 = listOf(
            PipelineEvent.PositionUpdate(sCm = 1000, vCms = 500, mode = "Normal")
        )
        val events2 = listOf(
            PipelineEvent.PositionUpdate(sCm = 2000, vCms = 600, mode = "Normal"),
            PipelineEvent.Arrival(stopIndex = 0, probability = 80)
        )

        val filename = "test_overwrite.jsonl"
        TraceStorageManager.saveTrace(events1, filename)

        val loaded1 = TraceStorageManager.loadTrace(filename)
        assertEquals(1, loaded1.size)

        TraceStorageManager.saveTrace(events2, filename)

        val loaded2 = TraceStorageManager.loadTrace(filename)
        assertEquals(2, loaded2.size)
    }
}
