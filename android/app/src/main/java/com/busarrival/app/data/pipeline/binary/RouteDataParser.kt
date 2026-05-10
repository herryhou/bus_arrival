package com.busarrival.app.data.pipeline.binary

import android.content.Context
import com.busarrival.app.domain.model.*
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.CRC32

/**
 * Binary route data parser (v5.1 sparse grid format).
 * Ported from crates/shared/src/binfile.rs
 *
 * Binary format:
 * - Magic: "BUSA" (4 bytes)
 * - Version: 5 (1 byte)
 * - CRC32: checksum (4 bytes)
 * - Header: origin_lat, origin_lon, avg_lat (12 bytes)
 * - Nodes: 24 bytes each
 * - Stops: 12 bytes each
 * - Grid: cell_size, rows, cols, cells...
 */
object RouteDataParser {

    private const val MAGIC = 0x42555341  // "BUSA"
    private const val VERSION = 5
    private const val NODE_SIZE = 24
    private const val STOP_SIZE = 12

    /**
     * Load route data from assets.
     */
    fun loadFromAssets(context: Context, filename: String): RouteData {
        val asset = context.assets.open(filename)
        val bytes = asset.readBytes()
        asset.close()
        return parse(bytes)
    }

    /**
     * Load route data from file.
     */
    fun loadFromFile(path: String): RouteData {
        val fis = FileInputStream(path)
        val bytes = fis.readBytes()
        fis.close()
        return parse(bytes)
    }

    /**
     * Parse route data from byte array.
     */
    fun parse(bytes: ByteArray): RouteData {
        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)

        // Verify magic
        val magic = buffer.int
        require(magic == MAGIC) { "Invalid magic: 0x${magic.toString(16)}" }

        // Verify version
        val version = buffer.get().toInt() and 0xFF
        require(version == VERSION) { "Unsupported version: $version" }

        // Verify CRC32
        val crcStored = buffer.int
        val crcCalc = calculateCrc(bytes, 9) // Skip magic (4) + version (1) + crc (4)
        require(crcCalc.toLong() == crcStored.toLong()) { "CRC mismatch: expected $crcStored, got $crcCalc" }

        // Read header
        val originLat = buffer.int
        val originLon = buffer.int
        val avgLat = buffer.int

        // Read node count
        val nodeCount = buffer.int

        // Read nodes
        val nodes = mutableListOf<RouteNode>()
        repeat(nodeCount) {
            nodes.add(RouteNode(
                xCm = buffer.int,
                yCm = buffer.int,
                cumDistCm = buffer.int,
                segLenMm = buffer.int,
                dxCm = buffer.short,
                dyCm = buffer.short,
                headingCdeg = buffer.short,
                pad = buffer.short
            ))
        }

        // Read stop count
        val stopCount = buffer.int

        // Read stops
        val stops = mutableListOf<Stop>()
        repeat(stopCount) {
            stops.add(Stop(
                progressCm = buffer.int,
                corridorStartCm = buffer.int,
                corridorEndCm = buffer.int
            ))
        }

        // Read spatial grid
        val grid = parseGrid(buffer)

        return RouteData(
            originLat = originLat,
            originLon = originLon,
            avgLat = avgLat,
            nodes = nodes,
            stops = stops,
            grid = grid
        )
    }

    /**
     * Parse spatial grid (v5.1 sparse format).
     */
    private fun parseGrid(buffer: ByteBuffer): SpatialGrid {
        val cellSizeCm = buffer.int
        val rows = buffer.int
        val cols = buffer.int

        val cells = mutableListOf<GridCell>()
        val totalCells = rows * cols

        repeat(totalCells) {
            val bitmask = buffer.long.toULong()
            val offsetCount = buffer.get().toInt() and 0xFF

            val offsets = mutableListOf<Int>()
            repeat(offsetCount) {
                offsets.add(buffer.short.toInt() and 0xFFFF)
            }

            cells.add(GridCell(
                bitmask = bitmask,
                offsets = offsets
            ))
        }

        return SpatialGrid(
            cellSizeCm = cellSizeCm,
            rows = rows,
            cols = cols,
            cells = cells
        )
    }

    /**
     * Calculate CRC32 checksum.
     */
    private fun calculateCrc(bytes: ByteArray, offset: Int): Int {
        val crc = CRC32()
        crc.update(bytes, offset, bytes.size - offset)
        return crc.value.toInt()
    }
}
