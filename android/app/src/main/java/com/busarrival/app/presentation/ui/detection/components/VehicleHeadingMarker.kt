package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.GpsFixState

@Composable
fun VehicleHeadingMarker(
    gpsFixState: GpsFixState,
    modifier: Modifier = Modifier
) {
    val bearing = when (gpsFixState) {
        is GpsFixState.Ready -> gpsFixState.bearing
        else -> null
    }

    if (bearing != null) {
        val markerSize = 32.dp
        val markerColor = MaterialTheme.colorScheme.primary

        Canvas(modifier = modifier.size(markerSize)) {
            val size = size.width
            val center = Offset(size / 2, size / 2)

            rotate(degrees = bearing.toFloat(), pivot = center) {
                // Draw arrow pointing up (north)
                val path = androidx.compose.ui.graphics.Path().apply {
                    moveTo(center.x, center.y - size / 2) // Top
                    lineTo(center.x - size / 4, center.y) // Left
                    lineTo(center.x, center.y + size / 4) // Bottom center (indent)
                    lineTo(center.x + size / 4, center.y) // Right
                    close()
                }

                drawPath(
                    path = path,
                    color = markerColor,
                    style = Stroke(width = 3f)
                )
            }
        }
    }
}
