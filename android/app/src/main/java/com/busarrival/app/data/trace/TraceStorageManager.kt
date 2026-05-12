package com.busarrival.app.data.trace

import android.content.Context
import com.busarrival.app.service.PipelineEvent
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Manages historical trip traces in JSONL format.
 * Traces are stored in app's internal storage under traces/ directory.
 */
object TraceStorageManager {

    private var _tracesDir: File? = null
    private val tracesDir: File
        get() = _tracesDir ?: File(context.filesDir, "traces").apply {
            mkdirs()
            _tracesDir = this
        }

    private lateinit var context: Context
    private lateinit var gson: Gson
    private val eventTypeToken = object : TypeToken<PipelineEvent>() {}.type

    /**
     * Initialize the manager with application context and Gson instance.
     */
    fun init(ctx: Context, gs: Gson) {
        context = ctx
        gson = gs
    }

    /**
     * Metadata for a trace file.
     */
    data class TraceMetadata(
        val filename: String,
        val recordCount: Int
    )

    /**
     * List available traces in storage.
     * Scans traces/ directory and returns metadata for each trace file.
     */
    suspend fun listTraces(): List<TraceMetadata> = withContext(Dispatchers.IO) {
        if (!::context.isInitialized) {
            throw IllegalStateException("TraceStorageManager not initialized. Call init() first.")
        }

        if (!tracesDir.exists()) return@withContext emptyList()

        tracesDir.listFiles()
            ?.filter { it.extension == "jsonl" }
            ?.mapNotNull { file ->
                try {
                    val events = loadTrace(file.name)
                    if (events.isNotEmpty()) {
                        TraceMetadata(
                            filename = file.name,
                            recordCount = events.size
                        )
                    } else null
                } catch (e: Exception) {
                    null
                }
            }
            ?.sortedByDescending { it.recordCount }
            ?: emptyList()
    }

    /**
     * Load trace by filename.
     * Reads and parses JSONL file line by line.
     */
    suspend fun loadTrace(filename: String): List<PipelineEvent> = withContext(Dispatchers.IO) {
        if (!::context.isInitialized) {
            throw IllegalStateException("TraceStorageManager not initialized. Call init() first.")
        }

        val file = File(tracesDir, filename)
        if (!file.exists()) {
            return@withContext emptyList()
        }

        try {
            file.readLines().mapNotNull { line ->
                if (line.isBlank()) null
                else {
                    try {
                        parseEvent(line)
                    } catch (e: Exception) {
                        null
                    }
                }
            }
        } catch (e: Exception) {
            emptyList()
        }
    }

    /**
     * Save trace to storage.
     * Writes events as JSONL format (one JSON object per line).
     */
    suspend fun saveTrace(
        events: List<PipelineEvent>,
        filename: String
    ): Result<Unit> = withContext(Dispatchers.IO) {
        if (!::context.isInitialized) {
            return@withContext Result.failure(
                IllegalStateException("TraceStorageManager not initialized. Call init() first.")
            )
        }

        try {
            val file = File(tracesDir, filename)
            val tempFile = File(tracesDir, "$filename.tmp")

            tempFile.writeText(
                events.joinToString("\n") { event ->
                    gson.toJson(event)
                } + "\n"
            )

            if (!tempFile.renameTo(file)) {
                tempFile.delete()
                return@withContext Result.failure(
                    IllegalStateException("Failed to write trace file")
                )
            }

            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    /**
     * Delete trace file.
     */
    suspend fun deleteTrace(filename: String): Result<Unit> = withContext(Dispatchers.IO) {
        if (!::context.isInitialized) {
            return@withContext Result.failure(
                IllegalStateException("TraceStorageManager not initialized. Call init() first.")
            )
        }

        try {
            val file = File(tracesDir, filename)
            if (file.exists()) {
                file.delete()
            }
            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    /**
     * Parse a single line as PipelineEvent.
     * Uses Gson's type token to handle sealed class hierarchy.
     */
    private fun parseEvent(line: String): PipelineEvent? {
        return gson.fromJson<PipelineEvent>(line, eventTypeToken)
    }
}
