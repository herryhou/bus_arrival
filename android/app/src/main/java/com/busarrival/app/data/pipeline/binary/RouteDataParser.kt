package com.busarrival.app.data.pipeline.binary

import android.content.Context
import com.busarrival.app.domain.model.*
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Binary route data parser (v5 sparse grid format).
 * Ported from crates/shared/src/binfile.rs
 *
 * Binary format (matches Rust pack_route_data):
 * - Magic: "BUSA" (4 bytes)
 * - Version: 5 (2 bytes, u16)
 * - node_count (2 bytes, u16)
 * - stop_count (1 byte, u8)
 * - padding (3 bytes) - align to 8 bytes
 * - x0_cm (4 bytes, i32)
 * - y0_cm (4 bytes, i32)
 * - lat_avg_deg (8 bytes, f64)
 * - nodes (24 bytes each)
 * - stops (12 bytes each)
 * - grid: cols (4), rows (4), grid_size_cm (4), bitmask, offsets, data
 * - CRC32 (4 bytes) at END
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
        // CRC is at the end (last 4 bytes)
        val dataBytes = bytes.copyOfRange(0, bytes.size - 4)
        val crcStored = ByteBuffer.wrap(bytes, bytes.size - 4, 4)
            .order(ByteOrder.LITTLE_ENDIAN)
            .int
        val crcCalc = crc32(dataBytes)

        android.util.Log.d("RouteDataParser", "File size: ${bytes.size}, Data size: ${dataBytes.size}")
        android.util.Log.d("RouteDataParser", "Stored CRC: $crcStored (0x${Integer.toHexString(crcStored)})")
        android.util.Log.d("RouteDataParser", "Calculated CRC: $crcCalc (0x${Integer.toHexString(crcCalc)})")

        require(crcCalc == crcStored) {
            "CRC mismatch: expected $crcStored, got $crcCalc"
        }

        val buffer = ByteBuffer.wrap(dataBytes).order(ByteOrder.LITTLE_ENDIAN)

        // Verify magic
        val magic = buffer.int
        require(magic == MAGIC) { "Invalid magic: 0x${magic.toString(16)}" }

        // Verify version (u16)
        val version = buffer.short.toInt() and 0xFFFF
        require(version == VERSION) { "Unsupported version: $version" }

        // Read node count (u16)
        val nodeCount = buffer.short.toInt() and 0xFFFF

        // Read stop count (u8)
        val stopCount = buffer.get().toInt() and 0xFF

        // Skip padding (3 bytes)
        buffer.position(buffer.position() + 3)

        // Read grid origin
        val x0Cm = buffer.int
        val y0Cm = buffer.int
        val latAvgDeg = buffer.double

        // Fixed origin for coordinate conversion (matches Rust preprocessor)
        // FIXED_ORIGIN_LAT_DEG = 20.0°N, FIXED_ORIGIN_LON_DEG = 120.0°E
        val originLatDeg = 20.0
        val originLonDeg = 120.0

        // Read nodes
        val nodes = mutableListOf<RouteNode>()
        repeat(nodeCount) {
            nodes.add(RouteNode(
                xCm = buffer.int,
                yCm = buffer.int,
                cumDistCm = buffer.int,
                segLenMm = buffer.int,
                dxCm = buffer.short.toShort(),
                dyCm = buffer.short.toShort(),
                headingCdeg = buffer.short.toShort(),
                pad = buffer.short.toShort()
            ))
        }

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
            originLat = (originLatDeg * 1e6).toInt(),  // Convert to fixed-point
            originLon = (originLonDeg * 1e6).toInt(),
            avgLat = (latAvgDeg * 1e6).toInt(),  // Convert to fixed-point
            x0Cm = x0Cm,  // Grid origin offset
            y0Cm = y0Cm,  // Grid origin offset
            nodes = nodes,
            stops = stops,
            grid = grid
        )
    }

    /**
     * Parse spatial grid (v5 sparse format).
     */
    private fun parseGrid(buffer: ByteBuffer): SpatialGrid {
        val cols = buffer.int
        val rows = buffer.int
        val cellSizeCm = buffer.int

        val totalCells = cols * rows
        val bitmaskBytes = (totalCells + 7) / 8
        val bitmask = ByteArray(bitmaskBytes)
        buffer.get(bitmask)

        // Count non-empty cells
        var nonEmptyCount = 0
        for (i in 0 until totalCells) {
            val byteIdx = i / 8
            val bitMask = 1 shl (i % 8)
            if (bitmask[byteIdx].toInt() and bitMask != 0) {
                nonEmptyCount++
            }
        }

        // Read offsets (u16 each, only for non-empty cells)
        val offsets = mutableListOf<Int>()
        repeat(nonEmptyCount) {
            offsets.add(buffer.short.toInt() and 0xFFFF)
        }

        // Add padding to ensure cell data is 2-byte aligned
        if (buffer.position() % 2 != 0) {
            buffer.get() // skip padding byte
        }

        // Data section starts here - save position
        val dataSectionStart = buffer.position()

        // Build cells list
        val cells = mutableListOf<GridCell>()
        var offsetIdx = 0
        for (i in 0 until totalCells) {
            val byteIdx = i / 8
            val bitMask = 1 shl (i % 8)
            val isEmpty = bitmask[byteIdx].toInt() and bitMask == 0

            if (isEmpty) {
                cells.add(GridCell(
                    bitmask = 0u,
                    offsets = emptyList()
                ))
            } else {
                // Seek to cell data using offset
                val cellOffset = offsets[offsetIdx++]
                buffer.position(dataSectionStart + cellOffset)

                // Read count and segment indices
                val count = buffer.short.toInt() and 0xFFFF
                val segmentIndices = mutableListOf<Int>()
                repeat(count) {
                    segmentIndices.add(buffer.short.toInt() and 0xFFFF)
                }

                cells.add(GridCell(
                    bitmask = 0uL,
                    offsets = segmentIndices
                ))
            }
        }

        return SpatialGrid(
            cellSizeCm = cellSizeCm,
            rows = rows,
            cols = cols,
            cells = cells
        )
    }

    /**
     * Compute CRC32 checksum using Java's CRC32 (same result as Rust implementation).
     */
    private fun crc32(data: ByteArray): Int {
        val crc = java.util.zip.CRC32()
        crc.update(data)
        return crc.value.toInt()
    }
}
