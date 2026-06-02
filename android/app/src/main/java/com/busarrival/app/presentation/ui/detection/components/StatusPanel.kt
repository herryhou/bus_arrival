package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.blur
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.GpsFixState
import com.busarrival.app.domain.model.ReplayState
import com.busarrival.app.presentation.viewmodel.DetectionUiState
import com.busarrival.app.service.PipelineEvent
import com.busarrival.app.presentation.ui.*
import java.util.Locale

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StatusPanel(
    uiState: DetectionUiState,
    events: List<PipelineEvent>,
    routeName: String?,
    gpsLoggingEnabled: Boolean,
    gpsFixState: GpsFixState,
    replayState: ReplayState,
    onStartStop: () -> Unit,
    onToggleGpsLogging: () -> Unit,
    onPlayPause: () -> Unit,
    onSeek: (Long) -> Unit,
    onSpeedChange: (Float) -> Unit,
    onExitSimulation: () -> Unit,
    modifier: Modifier = Modifier
) {
    val scrollState = rememberScrollState()

    Column(
        modifier = modifier
            .fillMaxHeight()
            .verticalScroll(scrollState)
            .padding(horizontal = 16.dp, vertical = 12.dp)
    ) {
        DashboardHeader(routeName = routeName ?: "Active route", isRunning = uiState.isRunning)

        Spacer(modifier = Modifier.height(12.dp))

        CurrentStopPanel(
            stopLabel = formatStopLabel(uiState.currentStop),
            stopState = formatStateLabel(uiState.currentStopState),
            mode = uiState.mode
        )

        Spacer(modifier = Modifier.height(12.dp))

        GpsStatusRow(
            gpsFixState = gpsFixState,
            positionCm = uiState.sCm,
            speedCms = uiState.vCms,
            modifier = Modifier.fillMaxWidth()
        )

        Spacer(modifier = Modifier.height(12.dp))

        if (replayState.traceFile != null) {
            TimelineScrubber(
                replayState = replayState,
                onPlayPause = onPlayPause,
                onSeek = onSeek,
                onSpeedChange = onSpeedChange,
                onExitSimulation = onExitSimulation,
                allowSeek = !replayState.traceFile.orEmpty().endsWith(".jsonl")
            )
        } else {
            ActionRow(
                isRunning = uiState.isRunning,
                gpsLoggingEnabled = gpsLoggingEnabled,
                onStartStop = onStartStop,
                onToggleGpsLogging = onToggleGpsLogging
            )
        }

        if (events.isNotEmpty()) {
            Spacer(modifier = Modifier.height(8.dp))
            RecentEvents(events = events.take(5))
        }
    }
}

@Composable
private fun DashboardHeader(routeName: String, isRunning: Boolean) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = routeName,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = TextHigh,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
        ActiveIndicator(isActive = isRunning)
    }
}

@Composable
private fun ActiveIndicator(isActive: Boolean) {
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val alpha by infiniteTransition.animateFloat(
        initialValue = 0.5f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(1000, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "pulse"
    )

    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = if (isActive) "Live" else "Ready",
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Medium,
            color = if (isActive) AccentPrimary else TextLow
        )
        Spacer(modifier = Modifier.width(8.dp))
        Box(
            modifier = Modifier
                .size(8.dp)
                .clip(CircleShape)
                .background(
                    if (isActive) {
                        AccentPrimary.copy(alpha = alpha)
                    } else {
                        Color.White.copy(alpha = 0.3f)
                    }
                )
        )
    }
}

@Composable
private fun CurrentStopPanel(stopLabel: String, stopState: String, mode: String) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        color = Color.Transparent,
        shadowElevation = 0.dp
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(
                    Brush.verticalGradient(
                        colors = listOf(
                            Surface1.copy(alpha = 0.12f),
                            Surface2.copy(alpha = 0.06f)
                        )
                    ),
                    shape = RoundedCornerShape(16.dp)
                )
                .padding(horizontal = 20.dp, vertical = 16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Current stop",
                    style = MaterialTheme.typography.labelMedium,
                    color = TextLow
                )
                Text(
                    text = stopLabel,
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    color = TextHigh,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    text = stopState,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = AccentPrimary,
                    maxLines = 1
                )
                Text(
                    text = mode,
                    style = MaterialTheme.typography.labelMedium,
                    color = TextLow,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ActionRow(
    isRunning: Boolean,
    gpsLoggingEnabled: Boolean,
    onStartStop: () -> Unit,
    onToggleGpsLogging: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Button(
            onClick = onStartStop,
            modifier = Modifier.weight(1f).height(56.dp),
            colors = if (isRunning) {
                ButtonDefaults.buttonColors(
                    containerColor = StateError,
                    contentColor = Color.White
                )
            } else {
                ButtonDefaults.buttonColors(
                    containerColor = AccentPrimary,
                    contentColor = Color.White
                )
            },
            shape = RoundedCornerShape(16.dp)
        ) {
            Text(
                if (isRunning) "Stop Detection" else "Start Detection",
                fontWeight = FontWeight.Bold
            )
        }

        GlassSwitch(
            checked = gpsLoggingEnabled,
            onToggle = onToggleGpsLogging,
            contentDescription = "Toggle GPS logging"
        )
    }
}

@Composable
private fun GlassSwitch(
    label: String = "GPS log",
    checked: Boolean,
    onToggle: () -> Unit,
    contentDescription: String
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = TextLow
        )
        Switch(
            checked = checked,
            onCheckedChange = { onToggle() },
            modifier = Modifier.semantics { this.contentDescription = contentDescription },
            colors = SwitchDefaults.colors(
                checkedThumbColor = AccentPrimary,
                checkedTrackColor = AccentContainer,
                uncheckedThumbColor = Color.White.copy(alpha = 0.5f),
                uncheckedTrackColor = Color.White.copy(alpha = 0.2f)
            )
        )
    }
}

@Composable
private fun RecentEvents(events: List<PipelineEvent>) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = "Recent events",
            style = MaterialTheme.typography.labelLarge,
            color = TextLow
        )
        Column(
            modifier = Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            events.forEach { event ->
                EventSummaryItem(event)
            }
        }
    }
}

internal fun formatStopLabel(currentStop: Int): String {
    return if (currentStop >= 0) "Stop ${currentStop + 1}" else "--"
}

internal fun formatDistance(positionCm: Int): String {
    return String.format(Locale.US, "%.1f m", positionCm / 100.0)
}

internal fun formatSpeed(speedCms: Int): String {
    return String.format(Locale.US, "%.1f km/h", speedCms * 0.036)
}

internal fun formatStateLabel(stopState: String): String {
    return when (stopState.uppercase(Locale.US)) {
        "ATSTOP" -> "At stop"
        "TRIPCOMPLETE" -> "Trip complete"
        "IDLE" -> "Idle"
        else -> {
            val spaced =
                stopState
                    .replace("_", " ")
                    .replace(Regex("([a-z])([A-Z])"), "$1 $2")
                    .lowercase(Locale.US)
            spaced.replaceFirstChar {
                if (it.isLowerCase()) it.titlecase(Locale.US) else it.toString()
            }
        }
    }
}

@Composable
private fun EventSummaryItem(event: PipelineEvent) {
    val markerColor = when (event) {
        is PipelineEvent.Arrival -> AccentPrimary
        is PipelineEvent.Departure -> StateError
        is PipelineEvent.PositionUpdate -> AccentPrimary
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                Brush.horizontalGradient(
                    colors = listOf(
                        Color.Transparent,
                        markerColor.copy(alpha = 0.15f)
                    )
                ),
                shape = RoundedCornerShape(12.dp)
            )
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(8.dp)
                .clip(CircleShape)
                .background(markerColor)
        )
        Spacer(modifier = Modifier.width(10.dp))
        Text(
            text = eventSummaryText(event),
            style = MaterialTheme.typography.bodyMedium,
            color = TextHigh.copy(alpha = 0.8f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
    }
}

private fun eventSummaryText(event: PipelineEvent): String {
    return when (event) {
        is PipelineEvent.Arrival -> "Arrival at stop ${event.stopIndex}"
        is PipelineEvent.Departure -> "Departure from stop ${event.stopIndex}"
        is PipelineEvent.PositionUpdate -> {
            "Position ${formatDistance(event.sCm)}, speed ${formatSpeed(event.vCms)}"
        }
    }
}
