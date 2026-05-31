package com.busarrival.app.presentation.ui.history.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import com.busarrival.app.presentation.viewmodel.LogManagerItem
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun GlassLogItem(
    item: LogManagerItem,
    onToggle: (String) -> Unit,
    onSimulate: (String) -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()

    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.98f else 1f,
        animationSpec = spring(
            dampingRatio = 0.8f,
            stiffness = Spring.StiffnessMedium
        ),
        label = "scale"
    )

    val borderColor by animateColorAsState(
        targetValue = when {
            item.isSelected -> Color(0xFF6C5CE7).copy(alpha = 0.5f)
            item.isActive -> Color(0xFF00CEC9).copy(alpha = 0.4f)
            else -> Color.White.copy(alpha = 0.08f)
        },
        animationSpec = tween(durationMillis = 300),
        label = "borderColor"
    )

    val containerAlpha by animateFloatAsState(
        targetValue = if (item.isSelected) 0.15f else 0.08f,
        animationSpec = tween(durationMillis = 300),
        label = "containerAlpha"
    )

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .scale(scale)
            .clip(RoundedCornerShape(16.dp))
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        Color.White.copy(alpha = containerAlpha),
                        Color.White.copy(alpha = 0.03f)
                    )
                )
            )
            .border(
                BorderStroke(
                    width = if (item.isSelected) 1.5.dp else 1.dp,
                    color = borderColor
                ),
                RoundedCornerShape(16.dp)
            )
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                onClick = { onToggle(item.reference) }
            )
            .padding(16.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Left: checkbox and info
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                modifier = Modifier.weight(1f)
            ) {
                // Custom checkbox
                Box(
                    modifier = Modifier
                        .size(26.dp)
                        .clip(RoundedCornerShape(7.dp))
                        .background(
                            when {
                                item.isSelected -> Color(0xFF6C5CE7)
                                else -> Color.White.copy(alpha = 0.1f)
                            }
                        )
                        .then(
                            if (!item.isSelected) {
                                Modifier.border(
                                    BorderStroke(
                                        1.5.dp,
                                        Color.White.copy(alpha = 0.25f)
                                    ),
                                    RoundedCornerShape(7.dp)
                                )
                            } else {
                                Modifier
                            }
                        )
                        .clickable { onToggle(item.reference) },
                    contentAlignment = Alignment.Center
                ) {
                    if (item.isSelected) {
                        Icon(
                            imageVector = Icons.Default.CheckCircle,
                            contentDescription = "Selected",
                            tint = Color.White,
                            modifier = Modifier.size(16.dp)
                        )
                    }
                }

                // Log info
                Column(
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    // Filename with active badge
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            text = item.filename,
                            style = MaterialTheme.typography.bodyLarge,
                            fontWeight = if (item.isActive) FontWeight.Bold else FontWeight.SemiBold,
                            color = if (item.isActive) Color(0xFF00CEC9) else Color.White.copy(alpha = 0.95f),
                            letterSpacing = (-0.01).em,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false)
                        )

                        if (item.isActive) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(4.dp)
                            ) {
                                Box(
                                    modifier = Modifier
                                        .size(6.dp)
                                        .clip(CircleShape)
                                        .background(Color(0xFF00CEC9))
                                )
                                Text(
                                    text = "ACTIVE",
                                    style = MaterialTheme.typography.labelSmall,
                                    fontWeight = FontWeight.Bold,
                                    color = Color(0xFF00CEC9),
                                    letterSpacing = (0.05).em
                                )
                            }
                        }
                    }

                    // Metadata row
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text(
                            text = formatTimestamp(item.modifiedAtMillis),
                            style = MaterialTheme.typography.bodyMedium,
                            color = Color.White.copy(alpha = 0.5f)
                        )
                        Text(
                            text = "•",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Color.White.copy(alpha = 0.3f)
                        )
                        Text(
                            text = formatSize(item.sizeBytes),
                            style = MaterialTheme.typography.bodyMedium,
                            color = Color.White.copy(alpha = 0.5f)
                        )
                    }
                }
            }

            // Right: simulate button
            IconButton(
                onClick = { onSimulate(item.reference) },
                enabled = item.canSimulate,
                modifier = Modifier
                    .size(44.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(
                        if (item.canSimulate)
                            Color(0xFF6C5CE7).copy(alpha = 0.2f)
                        else
                            Color.White.copy(alpha = 0.05f)
                    )
            ) {
                Icon(
                    imageVector = Icons.Default.PlayArrow,
                    contentDescription = "Simulate",
                    tint = if (item.canSimulate) Color(0xFF6C5CE7) else Color.White.copy(alpha = 0.3f),
                    modifier = Modifier.size(22.dp)
                )
            }
        }
    }
}

private fun formatTimestamp(millis: Long): String {
    val formatter = SimpleDateFormat("MMM dd, HH:mm", Locale.getDefault()).apply {
        timeZone = java.util.TimeZone.getDefault()
    }
    return formatter.format(Date(millis))
}

private fun formatSize(sizeBytes: Long): String {
    if (sizeBytes < 1024) return "$sizeBytes B"
    val kib = sizeBytes / 1024.0
    if (kib < 1024) return String.format(Locale.getDefault(), "%.1f KB", kib)
    val mib = kib / 1024.0
    return String.format(Locale.getDefault(), "%.1f MB", mib)
}
