package com.busarrival.app.service

import android.content.ContentResolver
import android.location.Location
import android.net.Uri
import android.provider.DocumentsContract
import java.io.BufferedWriter
import java.io.File
import java.io.FileOutputStream
import java.io.OutputStreamWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

interface GpsLogStore {
    fun create(routeId: String?, startedAtMillis: Long): GpsLogSession
}

interface GpsLogSession {
    val description: String
    fun append(line: String)
    fun close()
}

sealed class GpsLogStatus {
    data class Active(val description: String) : GpsLogStatus()
    data class Disabled(val reason: String) : GpsLogStatus()
}

class GpsLogWriter(
    private val store: GpsLogStore,
    private val clock: () -> Long = { System.currentTimeMillis() },
) {
    private var session: GpsLogSession? = null
    private var disabled = false

    val isActive: Boolean
        @Synchronized get() = session != null && !disabled

    @Synchronized
    fun open(routeId: String?): GpsLogStatus {
        close()
        disabled = false
        return try {
            val newSession = store.create(routeId, clock())
            session = newSession
            GpsLogStatus.Active(newSession.description)
        } catch (e: Exception) {
            disabled = true
            session = null
            GpsLogStatus.Disabled(e.message ?: "GPS logging unavailable")
        }
    }

    @Synchronized
    fun append(location: Location) {
        if (disabled) return
        val activeSession = session ?: return
        try {
            activeSession.append(location.toJsonLine())
        } catch (e: Exception) {
            disabled = true
            runCatching { activeSession.close() }
            session = null
        }
    }

    @Synchronized
    fun close() {
        val activeSession = session
        session = null
        if (activeSession != null) {
            runCatching { activeSession.close() }
        }
    }
}

class FileGpsLogStore(private val directory: File) : GpsLogStore {
    override fun create(routeId: String?, startedAtMillis: Long): GpsLogSession {
        directory.mkdirs()
        val file = File(directory, gpsLogFilename(routeId, startedAtMillis))
        val writer = BufferedWriter(OutputStreamWriter(FileOutputStream(file, true), Charsets.UTF_8))
        return WriterGpsLogSession(file.absolutePath, writer)
    }
}

class SafGpsLogStore(
    private val contentResolver: ContentResolver,
    private val treeUri: Uri,
) : GpsLogStore {
    override fun create(routeId: String?, startedAtMillis: Long): GpsLogSession {
        val rootUri = DocumentsContract.buildDocumentUriUsingTree(
            treeUri,
            DocumentsContract.getTreeDocumentId(treeUri)
        )
        val busArrivalUri = findOrCreateDirectory(rootUri, "BusArrival")
        val logsUri = findOrCreateDirectory(busArrivalUri, "gps-logs")
        val fileUri = DocumentsContract.createDocument(
            contentResolver,
            logsUri,
            "application/json",
            gpsLogFilename(routeId, startedAtMillis)
        ) ?: error("Failed to create GPS log document")
        val output = contentResolver.openOutputStream(fileUri, "wa")
            ?: error("Failed to open GPS log document")
        val writer = BufferedWriter(OutputStreamWriter(output, Charsets.UTF_8))
        return WriterGpsLogSession(fileUri.toString(), writer)
    }

    private fun findOrCreateDirectory(parentUri: Uri, name: String): Uri {
        findChild(parentUri, name, DocumentsContract.Document.MIME_TYPE_DIR)?.let { return it }
        return DocumentsContract.createDocument(
            contentResolver,
            parentUri,
            DocumentsContract.Document.MIME_TYPE_DIR,
            name
        ) ?: error("Failed to create $name directory")
    }

    private fun findChild(parentUri: Uri, name: String, mimeType: String): Uri? {
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(
            treeUri,
            DocumentsContract.getDocumentId(parentUri)
        )
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE
        )
        contentResolver.query(childrenUri, projection, null, null, null)?.use { cursor ->
            val idColumn = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            val nameColumn = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
            val mimeColumn = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_MIME_TYPE)
            while (cursor.moveToNext()) {
                if (cursor.getString(nameColumn) == name && cursor.getString(mimeColumn) == mimeType) {
                    return DocumentsContract.buildDocumentUriUsingTree(treeUri, cursor.getString(idColumn))
                }
            }
        }
        return null
    }
}

private class WriterGpsLogSession(
    override val description: String,
    private val writer: BufferedWriter,
) : GpsLogSession {
    override fun append(line: String) {
        writer.write(line)
        writer.newLine()
    }

    override fun close() {
        writer.flush()
        writer.close()
    }
}

private fun Location.toJsonLine(): String = buildString {
    append('{')
    append(""""t":""").append(time)
    append(""","lat":""").append(latitude)
    append(""","lon":""").append(longitude)
    if (hasAccuracy()) append(""","a":""").append(accuracy)
    if (hasSpeed()) append(""","s":""").append(speed)
    if (hasBearing()) append(""","b":""").append(bearing)
    provider?.let { append(""","p":"""").append(it.jsonEscaped()).append('"') }
    @Suppress("DEPRECATION")
    if (isFromMockProvider) append(""","m":true""")
    append('}')
}

private fun gpsLogFilename(routeId: String?, startedAtMillis: Long): String {
    val timestamp = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.US).apply {
        timeZone = TimeZone.getTimeZone("UTC")
    }.format(Date(startedAtMillis))
    val safeRoute = routeId
        ?.takeIf { it.isNotBlank() }
        ?.replace(Regex("[^A-Za-z0-9._-]"), "_")
    return if (safeRoute == null) {
        "gps-log-$timestamp.jsonl"
    } else {
        "gps-log-$safeRoute-$timestamp.jsonl"
    }
}

private fun String.jsonEscaped(): String = buildString {
    for (char in this@jsonEscaped) {
        when (char) {
            '\\' -> append("\\\\")
            '"' -> append("\\\"")
            '\b' -> append("\\b")
            '\u000C' -> append("\\f")
            '\n' -> append("\\n")
            '\r' -> append("\\r")
            '\t' -> append("\\t")
            else -> {
                if (char.code < 0x20) {
                    append("\\u")
                    append(char.code.toString(16).padStart(4, '0'))
                } else {
                    append(char)
                }
            }
        }
    }
}
