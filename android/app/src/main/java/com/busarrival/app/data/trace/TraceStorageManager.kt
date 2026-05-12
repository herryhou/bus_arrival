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

    private val tracesDir: File
        get() = File(context.filesDir, "traces").apply { mkdirs() }

    private lateinit var context: Context
    private lateinit var gson: Gson

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
        val duration: Long,
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
                        val duration = calculateDuration(events)
                        TraceMetadata(
                            filename = file.name,
                            duration = duration,
                            recordCount = events.size
                        )
                    } else null
                } catch (e: Exception) {
                    null
                }
            }
            ?.sortedByDescending { it.duration }
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
            file.writeText(
                events.joinToString("\n") { event ->
                    gson.toJson(event)
                }
            )
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
        val typeToken = object : TypeToken<PipelineEvent>() {}.type
        return gson.fromJson<PipelineEvent>(line, typeToken)
    }

    /**
     * Calculate duration from events based on PositionUpdate timestamps.
     * Duration is calculated from first to last position update.
     */
    private fun calculateDuration(events: List<PipelineEvent>): Long {
        // Note: Current PipelineEvent doesn't include timestamp
        // This is a placeholder implementation
        // In production, you'd add timestamp to PositionUpdate or track separately
        return 0L
    }
}
