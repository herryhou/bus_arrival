package com.busarrival.app.service

import android.content.Intent
import android.net.Uri
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class GpsLogActionsTest {

    @Test
    fun createShareIntentUsesFileProviderForFilePathReferences() {
        val context = RuntimeEnvironment.getApplication()
        val file = File(context.filesDir, "gps-log-share-test.jsonl").apply {
            writeText("""{"t":1}""")
        }

        val intent = GpsLogActions.createShareIntent(context, file.absolutePath)
        val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)

        assertEquals(Intent.ACTION_SEND, intent.action)
        assertEquals("application/json", intent.type)
        assertTrue(intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0)
        assertTrue(uri?.scheme == "content")
        assertTrue(!uri?.path.isNullOrBlank())
    }

    @Test
    fun deleteRemovesFilePathReference() {
        val context = RuntimeEnvironment.getApplication()
        val file = File(context.filesDir, "gps-log-delete-test.jsonl").apply {
            writeText("""{"t":1}""")
        }

        val deleted = GpsLogActions.delete(context, file.absolutePath)

        assertTrue(deleted)
        assertFalse(file.exists())
    }

    @Test
    fun createShareIntentPreservesContentUriReferences() {
        val context = RuntimeEnvironment.getApplication()
        val reference = "content://com.busarrival.app.documents/gps-logs/sample.jsonl"

        val intent = GpsLogActions.createShareIntent(context, reference)
        val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)

        assertEquals(reference, uri?.toString())
    }
}
