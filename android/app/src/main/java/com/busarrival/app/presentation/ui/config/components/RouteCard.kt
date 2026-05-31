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
                Color(0xFF6C5CE7).copy(alpha = 0.25f),
                Color(0xFF6C5CE7).copy(alpha = 0.15f)
            )
        )
    } else {
        Brush.verticalGradient(
            colors = listOf(
                Color.White.copy(alpha = 0.08f),
                Color.White.copy(alpha = 0.04f)
            )
        )
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp)
            .clip(RoundedCornerShape(16.dp))
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
                    color = Color.White
                )

                if (isActive) {
                    Spacer(modifier = Modifier.width(8.dp))
                    Box(
                        modifier = Modifier
                            .size(6.dp)
                            .clip(CircleShape)
                            .background(Color(0xFF00CEC9))
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
                    color = Color.White.copy(alpha = 0.6f)
                )
                Text(
                    text = formatDate(metadata.timestamp),
                    style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                    color = Color.White.copy(alpha = 0.5f)
                )
            }
        }

        IconButton(onClick = onDelete) {
            Icon(
                imageVector = Icons.Rounded.DeleteOutline,
                contentDescription = "Delete route",
                tint = Color(0xFFFF6B6B).copy(alpha = 0.8f)
            )
        }
    }
}

private fun formatDate(timestamp: Long): String {
    val sdf = SimpleDateFormat("MMM dd", Locale.getDefault())
    return sdf.format(Date(timestamp))
}
