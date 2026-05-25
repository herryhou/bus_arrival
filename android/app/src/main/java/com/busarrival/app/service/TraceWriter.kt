package com.busarrival.app.service

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.BufferedWriter
import java.io.File
import java.io.FileWriter

/**
 * Writes trace ticks to JSONL file (one JSON per line).
 * Zero allocation when disabled (enabled=false).
 */
class TraceWriter(
    private val file: File,
    private val enabled: Boolean = true
) {
    private val writer: BufferedWriter? = if (enabled) {
        BufferedWriter(FileWriter(file))
    } else null

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = false
    }

    /**
     * Write a single trace tick as JSON line.
     * No-op if disabled (null writer).
     */
    fun write(tick: TraceTick) {
        if (enabled) {
            writer?.write(json.encodeToString(tick))
            writer?.newLine()
        }
    }

    /**
     * Flush and close the writer.
     * Safe to call multiple times.
     */
    fun close() {
        writer?.close()
    }

    /**
     * Flush pending writes without closing.
     */
    fun flush() {
        writer?.flush()
    }
}
