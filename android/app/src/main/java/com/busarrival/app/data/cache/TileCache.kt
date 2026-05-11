package com.busarrival.app.data.cache

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.net.URL

/**
 * Disk cache for map tiles.
 * Enables offline map usage and reduces network requests.
 */
class TileCache(private val context: Context) {

    private val cacheDir: File = File(context.cacheDir, "map_tiles")

    init {
        if (!cacheDir.exists()) {
            cacheDir.mkdirs()
        }
    }

    /**
     * Get tile from disk cache.
     * @return Bitmap if cached, null otherwise
     */
    suspend fun getTile(z: Int, x: Int, y: Int): Bitmap? = withContext(Dispatchers.IO) {
        val file = getTileFile(z, x, y)
        if (file.exists()) {
            try {
                BitmapFactory.decodeFile(file.absolutePath)
            } catch (e: Exception) {
                android.util.Log.w("TileCache", "Failed to load cached tile", e)
                null
            }
        } else {
            null
        }
    }

    /**
     * Fetch tile from network and cache to disk.
     * @return Bitmap if successful, null otherwise
     */
    suspend fun fetchAndCacheTile(z: Int, x: Int, y: Int): Bitmap? = withContext(Dispatchers.IO) {
        val file = getTileFile(z, x, y)

        try {
            val url = URL("https://tile.openstreetmap.org/$z/$x/$y.png")
            val connection = url.openConnection()
            connection.connectTimeout = 10000
            connection.readTimeout = 10000
            connection.addRequestProperty("User-Agent", "BusArrival/1.0")

            val bitmap = BitmapFactory.decodeStream(connection.getInputStream())

            // Save to disk
            bitmap?.let { saveTile(file, it) }

            bitmap
        } catch (e: Exception) {
            android.util.Log.w("TileCache", "Failed to fetch tile $z/$x/$y", e)
            null
        }
    }

    /**
     * Save bitmap to disk cache.
     */
    private fun saveTile(file: File, bitmap: Bitmap) {
        try {
            FileOutputStream(file).use { out ->
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            }
        } catch (e: Exception) {
            android.util.Log.e("TileCache", "Failed to save tile", e)
        }
    }

    /**
     * Get cache file for a tile.
     */
    private fun getTileFile(z: Int, x: Int, y: Int): File {
        // Use directory structure: cacheDir/z/x/y.png
        val zDir = File(cacheDir, z.toString())
        val xDir = File(zDir, x.toString())
        if (!xDir.exists()) {
            xDir.mkdirs()
        }
        return File(xDir, "$y.png")
    }

    /**
     * Get total cache size in bytes.
     */
    fun getCacheSize(): Long {
        return cacheDir.walkTopDown()
            .filter { it.isFile }
            .sumOf { it.length() }
    }

    /**
     * Clear all cached tiles.
     */
    fun clearCache() {
        cacheDir.deleteRecursively()
        cacheDir.mkdirs()
    }

    /**
     * Get number of cached tiles.
     */
    fun getCachedTileCount(): Int {
        return cacheDir.walkTopDown()
            .filter { it.isFile }
            .count()
    }
}
