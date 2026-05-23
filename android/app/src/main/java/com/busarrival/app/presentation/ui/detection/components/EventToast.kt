package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowForward
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ExitToApp
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.EventHint
import com.busarrival.app.domain.model.HintType

@Composable
fun EventToast(
    hint: EventHint,
    modifier: Modifier = Modifier
) {
    val icon = when (hint.type) {
        HintType.APPROACHING -> Icons.Default.ArrowForward
        HintType.ARRIVING -> Icons.Default.LocationOn
        HintType.ATSTOP -> Icons.Default.Check
        HintType.DEPART -> Icons.Default.ExitToApp
    }

    val backgroundColor = when (hint.type) {
        HintType.APPROACHING -> Color(0xFF1565C0).copy(alpha = 0.9f) // Blue
        HintType.ARRIVING -> Color(0xFFF57C00).copy(alpha = 0.9f) // Orange
        HintType.ATSTOP -> Color(0xFF2E7D32).copy(alpha = 0.9f) // Green
        HintType.DEPART -> Color(0xFF6A1B9A).copy(alpha = 0.9f) // Purple
    }

    val message = when (hint.type) {
        HintType.APPROACHING -> "Approaching stop ${hint.stopIndex + 1}"
        HintType.ARRIVING -> "Arriving at stop ${hint.stopIndex + 1}"
        HintType.ATSTOP -> "At stop ${hint.stopIndex + 1}"
        HintType.DEPART -> "Departing stop ${hint.stopIndex + 1}"
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(backgroundColor, RoundedCornerShape(8.dp))
            .padding(12.dp),
        horizontalArrangement = Arrangement.Start,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = Color.White,
            modifier = Modifier.size(24.dp)
        )
        Spacer(modifier = Modifier.width(12.dp))
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = Color.White
        )
    }
}
