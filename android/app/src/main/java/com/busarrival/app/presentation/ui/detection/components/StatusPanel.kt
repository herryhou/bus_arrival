package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
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
import com.busarrival.app.domain.model.GpsFixState
import com.busarrival.app.presentation.viewmodel.DetectionUiState
import com.busarrival.app.service.PipelineEvent
import java.util.Locale

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StatusPanel(
        uiState: DetectionUiState,
        events: List<PipelineEvent>,
        routeName: String?,
        gpsLoggingEnabled: Boolean,
        gpsFixState: GpsFixState,
        onStartStop: () -> Unit,
        onToggleGpsLogging: () -> Unit,
        modifier: Modifier = Modifier
) {
    Card(
            modifier = modifier.fillMaxHeight().heightIn(max = 500.dp),
            shape = RoundedCornerShape(8.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
            elevation = CardDefaults.cardElevation(defaultElevation = 1.dp)
    ) {
        Column(
                modifier =
                        Modifier.padding(horizontal = 16.dp, vertical = 14.dp)
                                .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            DashboardHeader(routeName = routeName ?: "Active route", isRunning = uiState.isRunning)

            CurrentStopPanel(
                    stopLabel = formatStopLabel(uiState.currentStop),
                    stopState = formatStateLabel(uiState.currentStopState),
                    mode = uiState.mode
            )

            GpsStatusRow(
                    gpsFixState = gpsFixState,
                    positionCm = uiState.sCm,
                    speedCms = uiState.vCms,
                    modifier = Modifier.fillMaxWidth()
            )

            ActionRow(
                    isRunning = uiState.isRunning,
                    gpsLoggingEnabled = gpsLoggingEnabled,
                    onStartStop = onStartStop,
                    onToggleGpsLogging = onToggleGpsLogging
            )

            if (events.isNotEmpty()) {
                RecentEvents(events = events.take(5))
            }
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
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
            )
        }
        StatusChip(
                label = if (isRunning) "Live" else "Ready",
                selected = isRunning,
                color =
                        if (isRunning) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.tertiary
        )
    }
}

@Composable
private fun CurrentStopPanel(stopLabel: String, stopState: String, mode: String) {
    Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
            color = MaterialTheme.colorScheme.primaryContainer
    ) {
        Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                        text = "Current stop",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.72f)
                )
                Text(
                        text = stopLabel,
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                )
            }
            Column(horizontalAlignment = Alignment.End) {
                Text(
                        text = stopState,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        maxLines = 1
                )
                Text(
                        text = mode,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.72f),
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
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Button(
                onClick = onStartStop,
                modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                colors =
                        if (isRunning) {
                            ButtonDefaults.buttonColors(
                                    containerColor = MaterialTheme.colorScheme.error,
                                    contentColor = MaterialTheme.colorScheme.onError
                            )
                        } else {
                            ButtonDefaults.buttonColors()
                        }
        ) { Text(if (isRunning) "Stop Detection" else "Start Detection") }

        SecondarySwitch(
                label = "GPS log",
                checked = gpsLoggingEnabled,
                onToggle = onToggleGpsLogging,
                contentDescription = "Toggle GPS logging"
        )
    }
}

@Composable
private fun SecondarySwitch(
        label: String,
        checked: Boolean,
        onToggle: () -> Unit,
        contentDescription: String
) {
    Surface(shape = RoundedCornerShape(8.dp), color = MaterialTheme.colorScheme.background) {
        Row(
                modifier = Modifier.padding(start = 12.dp, end = 8.dp, top = 4.dp, bottom = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                    text = label,
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Switch(
                    checked = checked,
                    onCheckedChange = { onToggle() },
                    modifier = Modifier.semantics { this.contentDescription = contentDescription }
            )
        }
    }
}

@Composable
private fun StatusChip(label: String, selected: Boolean, color: Color) {
    FilterChip(
            selected = selected,
            onClick = {},
            label = { Text(label, maxLines = 1) },
            enabled = false,
            colors =
                    androidx.compose.material3.FilterChipDefaults.filterChipColors(
                            disabledContainerColor = color.copy(alpha = 0.14f),
                            disabledLabelColor = color
                    )
    )
}

@Composable
private fun RecentEvents(events: List<PipelineEvent>) {
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
                text = "Recent events",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(6.dp)
        ) { events.forEach { event -> EventSummaryItem(event) } }
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
    Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
            color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.58f)
    ) {
        Row(
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
        ) {
            val markerColor =
                    when (event) {
                        is PipelineEvent.Arrival -> Color(0xFF2E7D32)
                        is PipelineEvent.Departure -> MaterialTheme.colorScheme.error
                        is PipelineEvent.PositionUpdate -> MaterialTheme.colorScheme.tertiary
                    }
            Box(modifier = Modifier.size(8.dp).background(markerColor, CircleShape))
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                    text = eventSummaryText(event),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
            )
        }
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
