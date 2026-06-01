package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.animation.core.*
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
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.automirrored.filled.ExitToApp
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.EventHint
import com.busarrival.app.domain.model.HintType
import com.busarrival.app.presentation.ui.*

@Composable
fun EventToast(
    hint: EventHint,
    modifier: Modifier = Modifier
) {
    val icon = when (hint.type) {
        HintType.APPROACHING -> Icons.AutoMirrored.Filled.ArrowForward
        HintType.ARRIVING -> Icons.Default.LocationOn
        HintType.ATSTOP -> Icons.Default.Check
        HintType.DEPART -> Icons.AutoMirrored.Filled.ExitToApp
    }

    val (accentColor, backgroundColor) = when (hint.type) {
        HintType.APPROACHING -> AccentPrimary.copy(red = 0.4f, blue = 0.6f) to AccentContainer.copy(alpha = 0.3f)
        HintType.ARRIVING -> AccentPrimary.copy(green = 0.3f) to AccentContainer.copy(alpha = 0.4f)
        HintType.ATSTOP -> AccentPrimary to AccentContainer.copy(alpha = 0.3f)
        HintType.DEPART -> AccentPrimary to AccentContainer.copy(alpha = 0.2f)
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
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        backgroundColor,
                        backgroundColor.copy(alpha = 0.08f)
                    )
                ),
                shape = RoundedCornerShape(16.dp)
            )
            .padding(16.dp),
        horizontalArrangement = Arrangement.Start,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = accentColor,
            modifier = Modifier.size(24.dp)
        )
        Spacer(modifier = Modifier.width(12.dp))
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = TextHigh.copy(alpha = 0.9f),
            fontWeight = FontWeight.Medium
        )
    }
}
