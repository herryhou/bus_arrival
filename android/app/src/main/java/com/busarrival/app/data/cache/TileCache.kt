package com.busarrival.app.data.cache

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Disk cache for map tiles. Enables offline map usage and reduces network requests. */
class TileCache(private val context: Context) {
    companion object {
        private const val CACHE_DIR_NAME = "map_tiles"
        private const val CONNECT_TIMEOUT_MS = 5000
        private const val READ_TIMEOUT_MS = 5000
        private const val MAX_DISK_CACHE_BYTES = 128L * 1024L * 1024L
        private const val TILE_USER_AGENT = "BusArrival/1.0 (CartoDB Light All tiles)"
    }

    private val cacheDir: File = File(context.cacheDir, CACHE_DIR_NAME)
    private val fetchLocks = mutableMapOf<String, Mutex>()

    init {
        if (!cacheDir.exists()) {
            cacheDir.mkdirs()
        }
    }

    /**
     * Get tile from disk cache.
     * @return Bitmap if cached, null otherwise
     */
    suspend fun getTile(z: Int, x: Int, y: Int, resolution: TileResolution): Bitmap? =
            withContext(Dispatchers.IO) {
                val normalized = normalizeTileCoordinate(z, x, y) ?: return@withContext null
                val file = getTileFile(z, normalized.first, normalized.second, resolution)
                if (file.exists()) {
                    try {
                        BitmapFactory.decodeFile(file.absolutePath, bitmapOptions())
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
    suspend fun fetchAndCacheTile(
            z: Int,
            x: Int,
            y: Int,
            resolution: TileResolution
    ): Bitmap? =
            withContext(Dispatchers.IO) {
                val normalized = normalizeTileCoordinate(z, x, y) ?: return@withContext null
                val normalizedX = normalized.first
                val normalizedY = normalized.second
                val file = getTileFile(z, normalizedX, normalizedY, resolution)
                val lockKey = "$z/$normalizedX/$normalizedY@${resolution.scale}"
                val fetchLock = getFetchLock(lockKey)

                fetchLock.withLock {
                    getTile(z, x, y, resolution)?.let { return@withLock it }

                    try {
                        val url =
                                URL(
                                        "https://a.basemaps.cartocdn.com/light_all/$z/$normalizedX/$normalizedY${resolution.urlSuffix}.png"
                                )
                        val connection = url.openConnection() as HttpURLConnection
                        connection.connectTimeout = CONNECT_TIMEOUT_MS
                        connection.readTimeout = READ_TIMEOUT_MS
                        connection.instanceFollowRedirects = true
                        connection.addRequestProperty("User-Agent", TILE_USER_AGENT)

                        connection.useCaches = true
                        connection.connect()

                        if (connection.responseCode != HttpURLConnection.HTTP_OK) {
                            android.util.Log.w(
                                    "TileCache",
                                    "Tile fetch failed for $z/$normalizedX/$normalizedY status=${connection.responseCode}"
                            )
                            connection.disconnect()
                            return@withLock null
                        }

                        val bytes =
                                connection.inputStream.use { inputStream -> inputStream.readBytes() }
                        connection.disconnect()

                        if (bytes.isEmpty()) {
                            return@withLock null
                        }

                        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bitmapOptions())
                        if (bitmap != null) {
                            saveTile(file, bytes)
                            trimCacheIfNeeded()
                        }
                        bitmap
                    } catch (e: Exception) {
                        android.util.Log.w("TileCache", "Failed to fetch tile $z/$normalizedX/$normalizedY", e)
                        null
                    }
                }
            }

    /**
     * Load tile with fallback to lower resolutions for immediate display.
     * @return Pair of (bitmap to display, whether fallback was used)
     */
    suspend fun loadTileWithFallback(
            z: Int,
            x: Int,
            y: Int,
            targetRes: TileResolution
    ): Pair<Bitmap?, Boolean> =
            withContext(Dispatchers.IO) {
                // Check disk cache for exact resolution first
                var bitmap = getTile(z, x, y, targetRes)
                if (bitmap != null) return@withContext Pair(bitmap, false)

                // Try to fetch exact resolution from network
                bitmap = fetchAndCacheTile(z, x, y, targetRes)
                if (bitmap != null) return@withContext Pair(bitmap, false)

                // Fallback to lower resolutions for immediate display
                val fallbackOrder =
                        listOf(TileResolution.X2, TileResolution.X1)
                                .filter { it.scale < targetRes.scale }

                for (fallbackRes in fallbackOrder) {
                    bitmap = getTile(z, x, y, fallbackRes)
                    if (bitmap != null) {
                        return@withContext Pair(bitmap, true)
                    }
                }

                return@withContext Pair(null, false)
            }

    /** Save bitmap to disk cache. */
    private fun saveTile(file: File, bytes: ByteArray) {
        try {
            FileOutputStream(file).use { out ->
                out.write(bytes)
                out.flush()
            }
        } catch (e: Exception) {
            android.util.Log.e("TileCache", "Failed to save tile", e)
        }
    }

    /** Get cache file for a tile. */
    private fun getTileFile(z: Int, x: Int, y: Int, resolution: TileResolution): File {
        val zDir = File(cacheDir, z.toString())
        val xDir = File(zDir, x.toString())
        if (!xDir.exists()) {
            xDir.mkdirs()
        }
        return File(xDir, "$y@${resolution.scale}.png")
    }

    /** Get total cache size in bytes. */
    fun getCacheSize(): Long {
        return cacheDir.walkTopDown().filter { it.isFile }.sumOf { it.length() }
    }

    /** Clear all cached tiles including old @2x files. */
    fun clearCache() {
        cacheDir.deleteRecursively()
        cacheDir.mkdirs()
    }

    /**
     * Clear all cached tiles completely.
     */
    fun clearCacheAll() {
        cacheDir.walkTopDown().filter { it.isFile }.forEach { it.delete() }
        cacheDir.mkdirs()
    }

    /** Get number of cached tiles. */
    fun getCachedTileCount(): Int {
        return cacheDir.walkTopDown().filter { it.isFile }.count()
    }

    private fun bitmapOptions(): BitmapFactory.Options {
        val options = BitmapFactory.Options()
        options.inPreferredConfig = Bitmap.Config.RGB_565
        return options
    }

    private fun trimCacheIfNeeded() {
        val files = cacheDir.walkTopDown().filter { it.isFile }.toList()
        var totalBytes = files.sumOf { it.length() }
        if (totalBytes <= MAX_DISK_CACHE_BYTES) {
            return
        }

        files.sortedBy { it.lastModified() }.forEach { file ->
            if (totalBytes <= MAX_DISK_CACHE_BYTES) {
                return
            }
            val fileSize = file.length()
            if (file.delete()) {
                totalBytes -= fileSize
            }
        }
    }

    private fun normalizeTileCoordinate(z: Int, x: Int, y: Int): Pair<Int, Int>? {
        if (z < 0 || z > 30) {
            return null
        }

        val tileCount = 1 shl z
        if (y < 0 || y >= tileCount) {
            return null
        }

        val normalizedX = ((x % tileCount) + tileCount) % tileCount
        return Pair(normalizedX, y)
    }

    private fun getFetchLock(key: String): Mutex =
            synchronized(fetchLocks) { fetchLocks.getOrPut(key) { Mutex() } }
}
