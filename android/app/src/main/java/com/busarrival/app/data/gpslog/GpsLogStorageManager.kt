package com.busarrival.app.data.gpslog

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import com.busarrival.app.data.preferences.DetectionPreferences
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

data class GpsLogMetadata(
    val filename: String,
    val reference: String,
    val modifiedAtMillis: Long,
    val sizeBytes: Long,
    val isActive: Boolean
)

object GpsLogStorageManager {
    private const val SAF_ROOT_DIR = "BusArrival"
    private const val SAF_LOGS_DIR = "gps-logs"

    suspend fun listLogs(context: Context): List<GpsLogMetadata> = withContext(Dispatchers.IO) {
        val activeReference = DetectionPreferences(context).lastGpsLogReference
        val treeUri = DetectionPreferences(context).gpsLogTreeUri
        val logs =
            if (treeUri.isNullOrBlank()) {
                listFileLogs(context, activeReference)
            } else {
                listSafLogs(context, Uri.parse(treeUri), activeReference)
            }

        logs.sortedWith(
            compareByDescending<GpsLogMetadata> { it.modifiedAtMillis }.thenByDescending { it.filename }
        )
    }

    suspend fun loadLog(context: Context, reference: String): List<String> = withContext(Dispatchers.IO) {
        readLogLines(context, reference)
    }

    suspend fun deleteLog(context: Context, reference: String): Boolean = withContext(Dispatchers.IO) {
        if (isActive(context, reference)) return@withContext false

        val uri = runCatching { Uri.parse(reference) }.getOrNull()
        when (uri?.scheme) {
            "content" -> deleteContentUri(context, uri)
            else -> deleteFileReference(reference)
        }
    }

    fun isActive(context: Context, reference: String): Boolean {
        return reference == DetectionPreferences(context).lastGpsLogReference
    }

    private fun listFileLogs(context: Context, activeReference: String?): List<GpsLogMetadata> {
        val logDir = logDirectory(context)
        val files = logDir.listFiles().orEmpty()

        return files
            .asSequence()
            .filter { it.isFile && it.name.endsWith(".jsonl") }
            .map {
                GpsLogMetadata(
                    filename = it.name,
                    reference = it.absolutePath,
                    modifiedAtMillis = it.lastModified(),
                    sizeBytes = it.length(),
                    isActive = it.absolutePath == activeReference
                )
            }
            .toList()
    }

    private fun listSafLogs(
        context: Context,
        treeUri: Uri,
        activeReference: String?
    ): List<GpsLogMetadata> {
        val rootUri =
            DocumentsContract.buildDocumentUriUsingTree(
                treeUri,
                DocumentsContract.getTreeDocumentId(treeUri)
            )
        val busArrivalUri = findChildDirectory(context, treeUri, rootUri, SAF_ROOT_DIR) ?: return emptyList()
        val logsUri = findChildDirectory(context, treeUri, busArrivalUri, SAF_LOGS_DIR) ?: return emptyList()

        val childrenUri =
            DocumentsContract.buildChildDocumentsUriUsingTree(
                treeUri,
                DocumentsContract.getDocumentId(logsUri)
            )
        val projection =
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_LAST_MODIFIED,
                DocumentsContract.Document.COLUMN_SIZE,
                DocumentsContract.Document.COLUMN_MIME_TYPE
            )

        val logs = mutableListOf<GpsLogMetadata>()
        context.contentResolver.query(childrenUri, projection, null, null, null)?.use { cursor ->
            val documentIdIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            val displayNameIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
            val modifiedIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_LAST_MODIFIED)
            val sizeIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_SIZE)
            while (cursor.moveToNext()) {
                val filename = cursor.safeGetString(displayNameIndex) ?: continue
                if (!filename.endsWith(".jsonl")) continue

                val documentId = cursor.safeGetString(documentIdIndex) ?: continue
                val reference = DocumentsContract.buildDocumentUriUsingTree(treeUri, documentId).toString()
                logs.add(
                    GpsLogMetadata(
                        filename = filename,
                        reference = reference,
                        modifiedAtMillis = cursor.safeGetLong(modifiedIndex),
                        sizeBytes = cursor.safeGetLong(sizeIndex),
                        isActive = reference == activeReference
                    )
                )
            }
        }

        return logs
    }

    private fun findChildDirectory(
        context: Context,
        treeUri: Uri,
        parentUri: Uri,
        name: String
    ): Uri? {
        val childrenUri =
            DocumentsContract.buildChildDocumentsUriUsingTree(
                treeUri,
                DocumentsContract.getDocumentId(parentUri)
            )
        val projection =
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_MIME_TYPE
            )

        context.contentResolver.query(childrenUri, projection, null, null, null)?.use { cursor ->
            val documentIdIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            val displayNameIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
            val mimeTypeIndex = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)

            while (cursor.moveToNext()) {
                val displayName = cursor.safeGetString(displayNameIndex)
                val mimeType = cursor.safeGetString(mimeTypeIndex)
                if (displayName == name && mimeType == DocumentsContract.Document.MIME_TYPE_DIR) {
                    val documentId = cursor.safeGetString(documentIdIndex) ?: continue
                    return DocumentsContract.buildDocumentUriUsingTree(treeUri, documentId)
                }
            }
        }

        return null
    }

    private fun readLogLines(context: Context, reference: String): List<String> {
        val uri = runCatching { Uri.parse(reference) }.getOrNull()
        return when (uri?.scheme) {
            "content" -> context.contentResolver.openInputStream(uri)?.bufferedReader(Charsets.UTF_8)?.useLines { lines ->
                lines.filter { it.isNotBlank() }.toList()
            } ?: emptyList()
            else -> {
                val file = File(reference)
                if (!file.exists()) return emptyList()
                file.useLines { lines -> lines.filter { it.isNotBlank() }.toList() }
            }
        }
    }

    private fun deleteContentUri(context: Context, uri: Uri): Boolean {
        return runCatching {
            DocumentsContract.deleteDocument(context.contentResolver, uri)
        }.getOrElse {
            runCatching { context.contentResolver.delete(uri, null, null) > 0 }.getOrDefault(false)
        }
    }

    private fun deleteFileReference(reference: String): Boolean {
        val file = File(reference)
        return file.delete() || !file.exists()
    }

    private fun logDirectory(context: Context): File {
        return File(context.getExternalFilesDir(null) ?: context.filesDir, "gps-logs").apply { mkdirs() }
    }

    private fun android.database.Cursor.safeGetLong(columnIndex: Int): Long {
        if (columnIndex < 0) return 0L
        return runCatching { getLong(columnIndex) }.getOrDefault(0L)
    }

    private fun android.database.Cursor.safeGetString(columnIndex: Int): String? {
        if (columnIndex < 0) return null
        return runCatching { getString(columnIndex) }.getOrNull()
    }
}
