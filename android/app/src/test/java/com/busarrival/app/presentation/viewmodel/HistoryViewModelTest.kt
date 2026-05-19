package com.busarrival.app.presentation.viewmodel

import com.busarrival.app.data.preferences.DetectionPreferences
import java.io.File
import java.util.zip.ZipFile
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class HistoryViewModelTest {
    private val context = RuntimeEnvironment.getApplication()
    private val dispatcher = UnconfinedTestDispatcher()
    private val logDir =
        File(context.getExternalFilesDir(null) ?: context.filesDir, "gps-logs").apply {
            deleteRecursively()
            mkdirs()
        }

    @Before
    fun setUp() {
        Dispatchers.setMain(dispatcher)
        DetectionPreferences(context).apply {
            gpsLogTreeUri = null
            lastGpsLogReference = null
        }
        logDir.deleteRecursively()
        logDir.mkdirs()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun selectingLogsAndSharingBuildsSingleZipSelection() = runTest {
        val older =
            File(logDir, "gps-log-older.jsonl").apply {
                writeText("""{"t":1}""")
                setLastModified(1_000L)
            }
        val newer =
            File(logDir, "gps-log-newer.jsonl").apply {
                writeText("""{"t":2}""")
                setLastModified(2_000L)
            }

        val viewModel = HistoryViewModel(context)
        advanceUntilIdle()

        assertEquals(listOf("gps-log-newer.jsonl", "gps-log-older.jsonl"), viewModel.uiState.value.logs.map { it.filename })

        viewModel.selectAll()
        assertEquals(2, viewModel.uiState.value.selectedCount)
        assertTrue(viewModel.uiState.value.canDeleteSelected)

        val zipFile = viewModel.shareSelected()
        assertNotNull(zipFile)

        ZipFile(zipFile).use { zip ->
            val entries = zip.entries().asSequence().map { it.name }.toList()
            assertEquals(listOf("gps-log-newer.jsonl", "gps-log-older.jsonl"), entries)
        }

        assertTrue(older.exists())
        assertTrue(newer.exists())
    }

    @Test
    fun deleteSelectedBlocksTheActiveLog() = runTest {
        val active =
            File(logDir, "gps-log-active.jsonl").apply {
                writeText("""{"t":1}""")
                setLastModified(2_000L)
            }
        val inactive =
            File(logDir, "gps-log-inactive.jsonl").apply {
                writeText("""{"t":2}""")
                setLastModified(1_000L)
            }
        DetectionPreferences(context).lastGpsLogReference = active.absolutePath

        val viewModel = HistoryViewModel(context)
        advanceUntilIdle()

        viewModel.selectAll()
        viewModel.deleteSelected()

        assertTrue(active.exists())
        assertTrue(inactive.exists())
        assertEquals("Stop recording before deleting the active log.", viewModel.uiState.value.error)
    }
}
