package com.busarrival.app.data.storage

import android.content.Context
import android.net.Uri
import com.busarrival.app.data.pipeline.binary.RouteDataParser
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.RouteMetadata
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Manages route file storage and metadata.
 * Routes are copied from external storage to internal app storage.
 */
@Singleton
class RouteStorageManager @Inject constructor(
    @ApplicationContext private val context: Context,
    private val gson: Gson
) {
    private val routesDir: File
        get() = File(context.filesDir, "routes").apply { mkdirs() }

    private val metadataFile: File
        get() = File(routesDir, "metadata.json")

    /**
     * Copy route file from external URI to internal storage.
     * @return UUID of new route
     */
    suspend fun copyToInternal(sourceUri: Uri, name: String): Result<String> = withContext(Dispatchers.IO) {
        try {
            val uuid = UUID.randomUUID().toString()
            val destFile = File(routesDir, "$uuid.bin")

            context.contentResolver.openInputStream(sourceUri)?.use { input ->
                FileOutputStream(destFile).use { output ->
                    input.copyTo(output)
                }
            } ?: return@withContext Result.failure(Exception("Failed to open source file"))

            // Parse route to extract metadata
            val routeData = RouteDataParser.loadFromFile(destFile.absolutePath)
            val metadata = RouteMetadata(
                uuid = uuid,
                name = name,
                timestamp = System.currentTimeMillis(),
                stopCount = routeData.stops.size,
                filePath = destFile.absolutePath
            )

            // Save metadata
            saveMetadata(metadata)

            Result.success(uuid)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    /**
     * Load route data from file.
     */
    fun loadRoute(uuid: String): RouteData? {
        val file = File(routesDir, "$uuid.bin")
        return if (file.exists()) {
            RouteDataParser.loadFromFile(file.absolutePath)
        } else {
            null
        }
    }

    /**
     * Get all route metadata.
     */
    fun loadAllMetadata(): List<RouteMetadata> {
        if (!metadataFile.exists()) return emptyList()

        return try {
            val json = metadataFile.readText()
            val type = object : TypeToken<List<RouteMetadata>>() {}.type
            gson.fromJson(json, type) ?: emptyList()
        } catch (e: Exception) {
            emptyList()
        }
    }

    /**
     * Save metadata entry to metadata.json.
     */
    private fun saveMetadata(metadata: RouteMetadata) {
        val currentList = loadAllMetadata().toMutableList()
        currentList.add(metadata)

        val json = gson.toJson(currentList)
        metadataFile.writeText(json)
    }

    /**
     * Delete route file and metadata.
     */
    suspend fun deleteRoute(uuid: String): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            val file = File(routesDir, "$uuid.bin")
            if (file.exists()) {
                file.delete()
            }

            val currentList = loadAllMetadata().toMutableList()
            currentList.removeAll { it.uuid == uuid }

            val json = gson.toJson(currentList)
            metadataFile.writeText(json)

            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    /**
     * Get route file path.
     */
    fun getRoutePath(uuid: String): String? {
        val file = File(routesDir, "$uuid.bin")
        return if (file.exists()) file.absolutePath else null
    }
}
