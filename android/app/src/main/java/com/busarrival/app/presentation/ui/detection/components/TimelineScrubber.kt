package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
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
import com.busarrival.app.domain.model.ReplayState

/**
 * Timeline scrubber component for replay control with glassmorphism design.
 *
 * Features:
 * - Play/pause button with glow effect
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
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        Color.White.copy(alpha = 0.08f),
                        Color.White.copy(alpha = 0.03f)
                    )
                ),
                shape = RoundedCornerShape(16.dp)
            )
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
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
                    color = Color.White.copy(alpha = 0.6f)
                )
                Text(
                    text = replayTimeLabel(replayState),
                    style = MaterialTheme.typography.titleMedium,
                    color = Color.White,
                    fontWeight = FontWeight.SemiBold
                )
            }

            PlayPauseButton(
                isPlaying = replayState.isPlaying,
                onClick = onPlayPause
            )
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
                modifier = Modifier.fillMaxWidth(),
                colors = SliderDefaults.colors(
                    activeTrackColor = Color(0xFF6C5CE7),
                    inactiveTrackColor = Color.White.copy(alpha = 0.1f),
                    thumbColor = Color(0xFF6C5CE7),
                    activeTickColor = Color.Transparent,
                    inactiveTickColor = Color.Transparent
                )
            )
        } else {
            Text(
                text = "No trace loaded",
                style = MaterialTheme.typography.bodySmall,
                color = Color(0xFFFF6B6B)
            )
        }
    }
}

@Composable
private fun PlayPauseButton(
    isPlaying: Boolean,
    onClick: () -> Unit
) {
    val infiniteTransition = rememberInfiniteTransition(label = "glow")
    val glowAlpha by infiniteTransition.animateFloat(
        initialValue = 0.3f,
        targetValue = 0.6f,
        animationSpec = infiniteRepeatable(
            animation = tween(1000, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "glow"
    )

    Box(modifier = Modifier.size(56.dp)) {
        // Glow effect behind button
        Box(
            modifier = Modifier
                .size(56.dp)
                .blur(16.dp)
                .background(
                    Brush.radialGradient(
                        colors = listOf(
                            Color(0xFF6C5CE7).copy(alpha = glowAlpha),
                            Color.Transparent
                        )
                    ),
                    shape = CircleShape
                )
        )

        IconButton(
            onClick = onClick,
            modifier = Modifier
                .size(56.dp)
                .background(
                    Brush.linearGradient(
                        colors = listOf(
                            Color(0xFF6C5CE7),
                            Color(0xFF5A4AD1)
                        )
                    ),
                    shape = CircleShape
                )
        ) {
            Icon(
                imageVector = if (isPlaying) Icons.Default.Pause else Icons.Default.PlayArrow,
                contentDescription = if (isPlaying) "Pause" else "Play",
                tint = Color.White,
                modifier = Modifier.size(28.dp)
            )
        }
    }
}

/** Speed selector chips for playback control with glassmorphic style. */
@Composable
private fun SpeedSelector(currentSpeed: Float, onSpeedChange: (Float) -> Unit) {
    val speeds = listOf(SPEED_0_5X, SPEED_1X, SPEED_2X, SPEED_4X)
    val selectedIndex = speeds.indexOf(currentSpeed).coerceAtLeast(0)

    SingleChoiceSegmentedButtonRow {
        speeds.forEachIndexed { index, speed ->
            SegmentedButton(
                selected = index == selectedIndex,
                onClick = { onSpeedChange(speed) },
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.heightIn(min = 36.dp),
                colors = androidx.compose.material3.SegmentedButtonDefaults.colors(
                    activeContainerColor = Color(0xFF00CEC9).copy(alpha = 0.2f),
                    activeContentColor = Color(0xFF00CEC9),
                    inactiveContainerColor = Color.White.copy(alpha = 0.05f),
                    inactiveContentColor = Color.White.copy(alpha = 0.5f)
                )
            ) {
                Text(
                    text = speedLabel(speed),
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = if (index == selectedIndex) FontWeight.SemiBold else FontWeight.Normal
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
