package com.busarrival.app.presentation.ui.config.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.RouteMetadata
import com.busarrival.app.presentation.ui.AccentPrimary
import com.busarrival.app.presentation.ui.CardShape
import com.busarrival.app.presentation.ui.Surface1
import com.busarrival.app.presentation.ui.Surface2
import com.busarrival.app.presentation.ui.TextHigh
import com.busarrival.app.presentation.ui.TextLow
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun RouteCard(
    metadata: RouteMetadata,
    isActive: Boolean,
    onClick: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier
) {
    val cardGradient = if (isActive) {
        Brush.verticalGradient(
            colors = listOf(
                Surface1,
                Surface2
            )
        )
    } else {
        Brush.verticalGradient(
            colors = listOf(
                Surface2,
                Surface1
            )
        )
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp)
            .clip(CardShape)
            .background(cardGradient)
            .clickable(onClick = onClick)
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = metadata.name,
                    style = androidx.compose.material3.MaterialTheme.typography.titleMedium,
                    fontWeight = if (isActive) FontWeight.Bold else FontWeight.Medium,
                    color = TextHigh
                )

                if (isActive) {
                    Spacer(modifier = Modifier.width(8.dp))
                    Box(
                        modifier = Modifier
                            .size(6.dp)
                            .clip(CircleShape)
                            .background(AccentPrimary)
                    )
                }
            }

            Spacer(modifier = Modifier.height(6.dp))

            Row(
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "${metadata.stopCount} stops",
                    style = androidx.compose.material3.MaterialTheme.typography.bodyMedium,
                    color = TextLow
                )
                Text(
                    text = formatDate(metadata.timestamp),
                    style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                    color = TextLow
                )
            }
        }

        IconButton(onClick = onDelete) {
            Icon(
                imageVector = Icons.Rounded.DeleteOutline,
                contentDescription = "Delete route",
                tint = androidx.compose.material3.MaterialTheme.colorScheme.error
            )
        }
    }
}

private fun formatDate(timestamp: Long): String {
    val sdf = SimpleDateFormat("MMM dd", Locale.getDefault())
    return sdf.format(Date(timestamp))
}
