package com.busarrival.app.data.cache

/** Map tile resolution scales for different zoom levels. */
enum class TileResolution(val scale: Int, val urlSuffix: String) {
    X1(1, ""),
    X2(2, "@2x");

    companion object {
        /** Get appropriate resolution for zoom level. */
        fun forZoom(_zoom: Int): TileResolution = X2
    }
}
