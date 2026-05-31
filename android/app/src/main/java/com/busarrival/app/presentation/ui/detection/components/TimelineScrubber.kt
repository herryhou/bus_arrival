package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
                    Box(
                            modifier =
                                    Modifier.size(48.dp)
                                            .background(
                                                    color = MaterialTheme.colorScheme.primary,
                                                    shape = CircleShape
                                            ),
                            contentAlignment = Alignment.Center
                    ) {
                        Icon(
                                imageVector =
                                        if (replayState.isPlaying) Icons.Default.Pause
                                        else Icons.Default.PlayArrow,
                                contentDescription = if (replayState.isPlaying) "Pause" else "Play",
                                tint = MaterialTheme.colorScheme.onPrimary,
                                modifier = Modifier.size(28.dp)
                        )
                    }
                }
            }

            SpeedSelector(
                    currentSpeed = replayState.playbackSpeed,
                    onSpeedChange = onSpeedChange
            )

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

/** Speed selector chips for playback control. */
@Composable
private fun SpeedSelector(currentSpeed: Float, onSpeedChange: (Float) -> Unit) {
    val speeds = listOf(SPEED_0_5X, SPEED_1X, SPEED_2X, SPEED_4X)
    val selectedIndex = speeds.indexOf(currentSpeed).coerceAtLeast(0)

    SingleChoiceSegmentedButtonRow {
        speeds.forEachIndexed { index, speed ->
            SegmentedButton(
                    selected = index == selectedIndex,
                    onClick = { onSpeedChange(speed) },
                    shape =
                            RoundedCornerShape(
                                    when (index) {
                                        0 -> 8.dp
                                        speeds.size - 1 -> 8.dp
                                        else -> 0.dp
                                    }
                            ),
                    modifier = Modifier.heightIn(min = 32.dp)
            ) {
                Text(
                        text = speedLabel(speed),
                        style = MaterialTheme.typography.labelSmall
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

/** Format time in milliseconds to HH:MM:SS format. */
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
