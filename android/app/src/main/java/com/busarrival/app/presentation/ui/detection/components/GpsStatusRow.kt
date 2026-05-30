package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
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
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        val (statusText, statusColor) = gpsStatusTextAndColor(gpsFixState)
        MetricTile(
            label = "GPS",
            value = statusText,
            valueColor = statusColor,
            modifier = Modifier.weight(1f)
        )
        MetricTile(
            label = "Position",
            value = formatDistance(positionCm),
            modifier = Modifier.weight(1f)
        )
        MetricTile(
            label = "Speed",
            value = formatSpeed(speedCms),
            modifier = Modifier.weight(1f)
        )
    }
}

@Composable
private fun MetricTile(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    valueColor: Color = MaterialTheme.colorScheme.onSurface
) {
    Surface(
        modifier = modifier.heightIn(min = 56.dp),
        shape = RoundedCornerShape(8.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.68f)
    ) {
        Column(
            modifier = Modifier.padding(10.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                text = value,
                style = MaterialTheme.typography.bodyMedium,
                color = valueColor,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1
            )
        }
    }
}

@Composable
private fun gpsStatusTextAndColor(gpsFixState: GpsFixState): Pair<String, Color> {
    return when (gpsFixState) {
        is GpsFixState.NoSignal -> "No signal" to MaterialTheme.colorScheme.error
        is GpsFixState.Searching -> "Searching" to MaterialTheme.colorScheme.onSurfaceVariant
        is GpsFixState.Acquiring -> {
            "Acquiring ${gpsFixState.satellites}" to MaterialTheme.colorScheme.tertiary
        }
        is GpsFixState.Ready -> {
            val accText = "+/-${gpsFixState.accuracyM.toInt()}m"
            val satText = "${gpsFixState.satellites} sats"
            "$accText, $satText" to Color(0xFF2E7D32)
        }
    }
}
