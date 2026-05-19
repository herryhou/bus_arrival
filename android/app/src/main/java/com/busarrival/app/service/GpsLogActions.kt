package com.busarrival.app.service

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.FileProvider
import java.io.File

object GpsLogActions {

    fun createShareIntent(context: Context, reference: String): Intent {
        val uri = resolveShareUri(context, reference)
        return Intent(Intent.ACTION_SEND).apply {
            type = "application/json"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            clipData = ClipData.newUri(context.contentResolver, "GPS log", uri)
        }
    }

    fun share(context: Context, reference: String) {
        val intent = Intent.createChooser(createShareIntent(context, reference), "Share GPS log")
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
    }

    fun delete(context: Context, reference: String): Boolean {
        val uri = Uri.parse(reference)
        return when (uri.scheme) {
            "content" -> deleteContentUri(context, uri)
            else -> {
                val file = File(reference)
                file.delete() || !file.exists()
            }
        }
    }

    fun displayName(reference: String): String {
        val uri = Uri.parse(reference)
        return when (uri.scheme) {
            "content" -> uri.lastPathSegment?.substringAfterLast('/') ?: reference
            else -> File(reference).name.ifBlank { reference }
        }
    }

    private fun resolveShareUri(context: Context, reference: String): Uri {
        val uri = Uri.parse(reference)
        return when (uri.scheme) {
            "content" -> uri
            else -> FileProvider.getUriForFile(
                context,
                "${context.packageName}.fileprovider",
                File(reference)
            )
        }
    }

    private fun deleteContentUri(context: Context, uri: Uri): Boolean {
        return runCatching {
            android.provider.DocumentsContract.deleteDocument(context.contentResolver, uri)
        }.getOrElse {
            runCatching { context.contentResolver.delete(uri, null, null) > 0 }.getOrDefault(false)
        }
    }
}
