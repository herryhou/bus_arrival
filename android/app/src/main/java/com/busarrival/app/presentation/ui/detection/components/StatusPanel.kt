package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.busarrival.app.presentation.viewmodel.DetectionUiState
import com.busarrival.app.service.PipelineEvent

@Composable
fun StatusPanel(
        uiState: DetectionUiState,
        events: List<PipelineEvent>,
        routeName: String?,
        gpsLoggingEnabled: Boolean,
        onStartStop: () -> Unit,
        onToggleCamera: () -> Unit,
        onToggleGpsLogging: () -> Unit,
        modifier: Modifier = Modifier
) {
    Card(
            modifier = modifier.fillMaxWidth(),
            elevation = CardDefaults.cardElevation(defaultElevation = 4.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp).verticalScroll(rememberScrollState())) {
            // Route name
            routeName?.let {
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Start) {
                    Text(
                            text = it,
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.Bold,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                    )
                }
                Spacer(modifier = Modifier.height(4.dp))
            }

            // Current position info
            Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Column {
                    Text(
                            text = "Position:",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(text = "${uiState.sCm} cm", style = MaterialTheme.typography.bodyMedium)
                }

                Column {
                    Text(
                            text = "Speed:",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(text = "${uiState.vCms} cm/s", style = MaterialTheme.typography.bodyMedium)
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                CompactSummaryRow(
                        stopLabel = debugStopLabel(uiState.currentStop),
                        stopState = uiState.currentStopState,
                        mode = uiState.mode
                )

                TelemetryGrid(
                        cameraFollow = if (uiState.isCameraFollowEnabled) "On" else "Off",
                        running = if (uiState.isRunning) "Yes" else "No",
                )
            }

            Spacer(modifier = Modifier.height(16.dp))

            Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
            ) {
                Button(onClick = onStartStop, modifier = Modifier.weight(1f)) {
                    Text(if (uiState.isRunning) "Stop Detection" else "Start Detection")
                }

                Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                            text = "GPS log",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Switch(
                            checked = gpsLoggingEnabled,
                            onCheckedChange = { onToggleGpsLogging() },
                            modifier =
                                    Modifier.semantics { contentDescription = "Toggle GPS logging" }
                    )
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Recent events
            if (events.isNotEmpty()) {
                Text(
                        text = "Recent Events",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(8.dp))

                Column(
                        modifier = Modifier.fillMaxWidth(),
                        verticalArrangement = Arrangement.spacedBy(4.dp)
                ) { events.take(5).forEach { event -> EventSummaryItem(event) } }
            }
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
            text = text,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
    )
}

@Composable
private fun CompactSummaryRow(stopLabel: String, stopState: String, mode: String) {
    Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically
    ) {
        CompactChip(label = stopLabel, background = MaterialTheme.colorScheme.surfaceVariant)
        CompactChip(label = stopState, background = debugStateColor(stopState))
        CompactChip(label = mode, background = MaterialTheme.colorScheme.secondaryContainer)
    }
}

@Composable
private fun CompactChip(label: String, background: Color) {
    Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = Color.White,
            maxLines = 1,
            modifier =
                    Modifier.background(color = background, shape = RoundedCornerShape(999.dp))
                            .padding(horizontal = 10.dp, vertical = 4.dp)
    )
}

@Composable
private fun TelemetryGrid(cameraFollow: String, running: String) {
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        DebugLine(label = "Camera follow", value = cameraFollow)
        DebugLine(label = "Running", value = running)
    }
}

@Composable
private fun DebugLine(label: String, value: String) {
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(
                text = label,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
                text = value,
                style = MaterialTheme.typography.bodySmall,
                fontWeight = FontWeight.Medium
        )
    }
}

private fun debugStopLabel(currentStop: Int): String {
    return if (currentStop >= 0) "Stop ${currentStop + 1}" else "No active stop"
}

private fun debugStateColor(stopState: String): Color {
    return when (stopState.uppercase()) {
        "APPROACHING" -> Color(0xFF1565C0)
        "ARRIVING" -> Color(0xFFF57C00)
        "ATSTOP" -> Color(0xFF2E7D32)
        "DEPARTED" -> Color(0xFF6A1B9A)
        "TRIPCOMPLETE" -> Color(0xFF424242)
        "IDLE" -> Color(0xFF37474F)
        else -> Color(0xFF263238)
    }
}

@Composable
private fun EventSummaryItem(event: PipelineEvent) {
    Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically
    ) {
        when (event) {
            is PipelineEvent.Arrival -> {
                Spacer(modifier = Modifier.size(8.dp).background(Color.Green, CircleShape))
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                        text = "Arrival at stop ${event.stopIndex}",
                        style = MaterialTheme.typography.bodySmall
                )
            }
            is PipelineEvent.Departure -> {
                Spacer(modifier = Modifier.size(8.dp).background(Color.Red, CircleShape))
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                        text = "Departure from stop ${event.stopIndex}",
                        style = MaterialTheme.typography.bodySmall
                )
            }
            is PipelineEvent.PositionUpdate -> {
                Spacer(modifier = Modifier.size(8.dp).background(Color.Blue, CircleShape))
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                        text = "Pos: ${event.sCm} cm, Vel: ${event.vCms} cm/s",
                        style = MaterialTheme.typography.bodySmall
                )
            }
        }
    }
}
