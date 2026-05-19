package com.busarrival.app.service

import android.content.ClipData
import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import com.busarrival.app.data.gpslog.GpsLogMetadata
import java.io.File
import java.io.FileOutputStream
import java.nio.charset.StandardCharsets
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

object GpsLogArchive {
    private const val ARCHIVE_DIR = "gps-log-archives"

    fun createZip(context: Context, selected: List<GpsLogMetadata>): File {
        val archiveDir = File(context.cacheDir, ARCHIVE_DIR).apply { mkdirs() }
        val archiveFile = File(archiveDir, zipFilename())

        ZipOutputStream(FileOutputStream(archiveFile)).use { zip ->
            selected.forEach { metadata ->
                val entry = ZipEntry(metadata.filename)
                zip.putNextEntry(entry)
                zip.write(readLogText(context, metadata.reference).toByteArray(StandardCharsets.UTF_8))
                zip.closeEntry()
            }
        }

        return archiveFile
    }

    fun shareZip(context: Context, zipFile: File): Intent {
        val uri =
            FileProvider.getUriForFile(
                context,
                "${context.packageName}.fileprovider",
                zipFile
            )

        return Intent(Intent.ACTION_SEND).apply {
            type = "application/zip"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            clipData = ClipData.newUri(context.contentResolver, zipFile.name, uri)
        }
    }

    private fun readLogText(context: Context, reference: String): String {
        val lines = GpsLogStorageReader.readLines(context, reference)
        return buildString {
            lines.forEachIndexed { index, line ->
                if (index > 0) append('\n')
                append(line)
            }
            if (lines.isNotEmpty()) append('\n')
        }
    }

    private fun zipFilename(): String {
        val timestamp =
            SimpleDateFormat("yyyyMMdd-HHmmss", Locale.US).apply {
                timeZone = TimeZone.getTimeZone("UTC")
            }.format(Date())
        return "gps-logs-$timestamp.zip"
    }
}

private object GpsLogStorageReader {
    fun readLines(context: Context, reference: String): List<String> {
        val uri = runCatching { android.net.Uri.parse(reference) }.getOrNull()
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
}
