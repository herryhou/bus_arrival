package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
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

@Composable
fun TimelineScrubber(
    replayState: ReplayState,
    onPlayPause: () -> Unit,
    onSeek: (Long) -> Unit,
    onSpeedChange: (Float) -> Unit,
    onToggleCameraFollow: () -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        elevation = CardDefaults.cardElevation(defaultElevation = 4.dp)
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            // Play/Pause button
            IconButton(onClick = onPlayPause) {
                Icon(
                    imageVector = if (replayState.isPlaying) {
                        Icons.Default.Pause
                    } else {
                        Icons.Default.PlayArrow
                    },
                    contentDescription = if (replayState.isPlaying) "Pause" else "Play",
                    tint = MaterialTheme.colorScheme.primary
                )
            }

            // Speed selector
            SpeedSelector(
                currentSpeed = replayState.playbackSpeed,
                onSpeedChange = onSpeedChange
            )

            // Camera follow toggle
            IconButton(onClick = onToggleCameraFollow) {
                Icon(
                    imageVector = Icons.Default.CameraAlt,
                    contentDescription = "Toggle camera follow",
                    tint = if (replayState.cameraFollowEnabled) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    }
                )
            }

            // Time display and progress
            Row(
                modifier = Modifier.weight(1f),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = formatTime(replayState.currentTime),
                    style = MaterialTheme.typography.bodyMedium
                )
                Text(
                    text = " / ",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = formatTime(replayState.traceDuration),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                if (replayState.traceDuration > 0) {
                    val progress = (replayState.currentTime.toFloat() / replayState.traceDuration * 100).toInt()
                    Text(
                        text = " [$progress%]",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }

        // Timeline slider
        if (replayState.traceDuration > 0) {
            Slider(
                value = replayState.currentTime.toFloat(),
                onValueChange = { value -> onSeek(value.toLong()) },
                valueRange = 0f..replayState.traceDuration.toFloat(),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp)
            )
        } else {
            Text(
                text = "No trace loaded",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
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
        horizontalArrangement = Arrangement.spacedBy(4.dp),
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
                contentPadding = androidx.compose.foundation.layout.padding(horizontal = 4.dp, vertical = 4.dp)
            ) {
                Text(
                    text = "${speed}x",
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
