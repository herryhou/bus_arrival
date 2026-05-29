package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

internal val vehicleHeadingMarkerSize = 64.dp
internal val vehicleHeadingIconSize = vehicleHeadingMarkerSize / 2f

@Composable
fun VehicleHeadingMarker(bearing: Float?, modifier: Modifier = Modifier) {
    val arrowColor = Color(0xFF2E7D32) // Material Green 800
    val questionColor = Color.Red

    if (bearing != null) {
        Box(
                modifier = modifier.size(vehicleHeadingMarkerSize),
                contentAlignment = Alignment.Center
        ) {
            Icon(
                    imageVector = vehicleNavigationIcon,
                    contentDescription = "Vehicle heading",
                    tint = arrowColor,
                    modifier =
                            Modifier.size(vehicleHeadingIconSize)
                                    .rotate(vehicleHeadingRotationDegrees(bearing))
            )
        }
    } else {
        Box(
                modifier = modifier.size(vehicleHeadingMarkerSize),
                contentAlignment = Alignment.Center
        ) {
            Text(
                    text = "?",
                    color = questionColor,
                    fontSize = 28.sp,
                    fontWeight = androidx.compose.ui.text.font.FontWeight.Bold
            )
        }
    }
}

internal fun vehicleHeadingRotationDegrees(bearing: Float): Float = bearing

internal fun vehicleHeadingMarkerTopLeft(
        gpsPosition: Offset,
        bearing: Float?,
        markerPx: Float,
        iconPx: Float
): Offset {
    val markerCenter = markerPx / 2f
    return Offset(gpsPosition.x - markerCenter, gpsPosition.y - markerCenter)
}

private val vehicleNavigationIcon: ImageVector =
        ImageVector.Builder(
                        name = "VehicleNavigation",
                        defaultWidth = 24.dp,
                        defaultHeight = 24.dp,
                        viewportWidth = 24f,
                        viewportHeight = 24f
                )
                .apply {
                    // White stroke edge (drawn first, behind fill)
                    // path(
                    //     fill = null,
                    //     stroke = SolidColor(Color.White),
                    //     strokeLineWidth = 2f,
                    //     pathFillType = PathFillType.NonZero
                    // ) {
                    //     moveTo(12f, 2f)
                    //     lineTo(4.5f, 20.29f)
                    //     lineTo(5.21f, 21f)
                    //     lineTo(12f, 18f)
                    //     lineTo(18.79f, 21f)
                    //     lineTo(19.5f, 20.29f)
                    //     close()
                    // }
                    // Fill (tinted by Icon's tint parameter)
                    path(fill = SolidColor(Color.Black), pathFillType = PathFillType.NonZero) {
                        moveTo(12f, 2f)
                        lineTo(4.5f, 20.29f)
                        lineTo(5.21f, 21f)
                        lineTo(12f, 18f)
                        lineTo(18.79f, 21f)
                        lineTo(19.5f, 20.29f)
                        close()
                    }
                }
                .build()
