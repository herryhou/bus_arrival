package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.ReplayState

/**
 * Timeline scrubber component for replay control.
 *
 * Features:
 * - Play/pause button
 * - Playback speed selector (0.5x, 1x, 2x, 4x)
 * - Camera follow toggle
 * - Timeline slider with position display
 * - Time display (current/total)
 * - Progress percentage
 */

private const val SPEED_0_5X = 0.5f
private const val SPEED_1X = 1f
private const val SPEED_2X = 2f
private const val SPEED_4X = 4f

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TimelineScrubber(
    replayState: ReplayState,
    onPlayPause: () -> Unit,
    onSeek: (Long) -> Unit,
    onSpeedChange: (Float) -> Unit,
    onToggleCameraFollow: () -> Unit,
    allowSeek: Boolean = true,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(8.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp)
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Replay",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = replayTimeLabel(replayState),
                        style = MaterialTheme.typography.titleMedium
                    )
                }

                IconButton(onClick = onPlayPause) {
                    Icon(
                        imageVector = if (replayState.isPlaying) Icons.Default.Close else Icons.Default.PlayArrow,
                        contentDescription = if (replayState.isPlaying) "Pause" else "Play",
                        tint = MaterialTheme.colorScheme.primary
                    )
                }
            }

            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                SpeedSelector(
                    currentSpeed = replayState.playbackSpeed,
                    onSpeedChange = onSpeedChange
                )

                FilterChip(
                    selected = replayState.cameraFollowEnabled,
                    onClick = onToggleCameraFollow,
                    label = { Text("Camera follow") },
                    leadingIcon = {
                        Icon(
                            imageVector = Icons.Default.Check,
                            contentDescription = null
                        )
                    }
                )
            }

            if (replayState.traceDuration > 0) {
                Slider(
                    value = replayState.currentTime.toFloat(),
                    onValueChange = { value -> onSeek(value.toLong()) },
                    valueRange = 0f..replayState.traceDuration.toFloat(),
                    enabled = allowSeek,
                    modifier = Modifier.fillMaxWidth()
                )
            } else {
                Text(
                    text = "No trace loaded",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
            }
        }
    }
}

/**
 * Speed selector buttons for playback control.
 */
@Composable
private fun SpeedSelector(
    currentSpeed: Float,
    onSpeedChange: (Float) -> Unit
) {
    val speeds = listOf(SPEED_0_5X, SPEED_1X, SPEED_2X, SPEED_4X)

    Row(
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        speeds.forEach { speed ->
            androidx.compose.material3.Button(
                onClick = { onSpeedChange(speed) },
                colors = androidx.compose.material3.ButtonDefaults.buttonColors(
                    containerColor = if (currentSpeed == speed) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.surfaceVariant
                    },
                    contentColor = if (currentSpeed == speed) {
                        MaterialTheme.colorScheme.onPrimary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    }
                ),
                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                modifier = Modifier.heightIn(min = 40.dp),
                shape = RoundedCornerShape(8.dp)
            ) {
                Text(
                    text = speedLabel(speed),
                    style = if (currentSpeed == speed) {
                        MaterialTheme.typography.labelMedium
                    } else {
                        MaterialTheme.typography.labelSmall
                    }
                )
            }
        }
    }
}

private fun speedLabel(speed: Float): String {
    return if (speed == speed.toInt().toFloat()) {
        "${speed.toInt()}x"
    } else {
        "${speed}x"
    }
}

private fun replayTimeLabel(replayState: ReplayState): String {
    if (replayState.traceDuration <= 0) return "No trace loaded"
    val progress = (replayState.currentTime.toFloat() / replayState.traceDuration * 100).toInt()
    return "${formatTime(replayState.currentTime)} / ${formatTime(replayState.traceDuration)}  $progress%"
}

/**
 * Format time in milliseconds to HH:MM:SS format.
 */
private fun formatTime(timeMs: Long): String {
    val totalSeconds = timeMs / 1000
    val hours = totalSeconds / 3600
    val minutes = (totalSeconds % 3600) / 60
    val seconds = totalSeconds % 60

    return if (hours > 0) {
        String.format("%02d:%02d:%02d", hours, minutes, seconds)
    } else {
        String.format("%02d:%02d", minutes, seconds)
    }
}
