package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.GpsFixState

@Composable
fun GpsStatusRow(
    gpsFixState: GpsFixState,
    positionCm: Int,
    speedCms: Int,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier,
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        // GPS Status Column
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = "GPS:",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            val (statusText, statusColor) = when (gpsFixState) {
                is GpsFixState.NoSignal -> "No signal" to Color.Gray
                is GpsFixState.Searching -> "Searching..." to Color.Gray
                is GpsFixState.Acquiring -> "Acquiring (${gpsFixState.satellites} sats)" to Color.Gray
                is GpsFixState.Ready -> {
                    val accText = "±${gpsFixState.accuracyM.toInt()}m"
                    val satText = "${gpsFixState.satellites}sats"
                    "$accText, $satText" to Color(0xFF2E7D32)
                }
            }
            Text(
                text = statusText,
                style = MaterialTheme.typography.bodyMedium,
                color = statusColor,
                fontWeight = if (gpsFixState is GpsFixState.Ready) FontWeight.Bold else FontWeight.Normal
            )
        }

        // Position Column
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = "Position:",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                text = "${positionCm} cm",
                style = MaterialTheme.typography.bodyMedium
            )
        }

        // Speed Column
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = "Speed:",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                text = "${speedCms} cm/s",
                style = MaterialTheme.typography.bodyMedium
            )
        }
    }
}
