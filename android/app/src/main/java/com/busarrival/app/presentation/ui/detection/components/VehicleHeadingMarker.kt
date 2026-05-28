package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun VehicleHeadingMarker(
    bearing: Float?,
    modifier: Modifier = Modifier
) {
    val markerSize = 32.dp
    val arrowColor = Color(0xFF2196F3)
    val questionColor = Color.Red

    if (bearing != null) {
        Icon(
            imageVector = Icons.AutoMirrored.Filled.ArrowForward,
            contentDescription = "Vehicle heading",
            tint = arrowColor,
            modifier = modifier
                .size(markerSize)
                .rotate(vehicleHeadingRotationDegrees(bearing))
        )
    } else {
        Box(
            modifier = modifier.size(markerSize),
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

internal fun vehicleHeadingRotationDegrees(bearing: Float): Float = bearing - 90f
