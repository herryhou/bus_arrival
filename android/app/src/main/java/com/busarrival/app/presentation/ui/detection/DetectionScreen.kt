package com.busarrival.app.presentation.ui.detection

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import com.busarrival.app.presentation.viewmodel.DetectionUiState
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import com.busarrival.app.service.PipelineEvent
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.rememberMultiplePermissionsState

@OptIn(ExperimentalPermissionsApi::class)
@Composable
fun DetectionScreen(
    navController: NavHostController,
    viewModel: DetectionViewModel
) {
    val uiState by viewModel.uiState.collectAsState()
    val events by viewModel.events.collectAsState()
    val context = LocalContext.current

    // Permission handling
    val locationPermissions = rememberMultiplePermissionsState(
        permissions = listOf(
            android.Manifest.permission.ACCESS_FINE_LOCATION,
            android.Manifest.permission.ACCESS_COARSE_LOCATION
        )
    )

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            Text(text = "Bus Arrival Detection")
            Spacer(modifier = Modifier.height(16.dp))

            if (!locationPermissions.allPermissionsGranted) {
                Text(text = "Location permission required")
                Spacer(modifier = Modifier.height(16.dp))
                Button(onClick = { locationPermissions.launchMultiplePermissionRequest() }) {
                    Text("Grant Permission")
                }
            } else {
                // Status
                Text(text = "Status: ${if (uiState.isRunning) "Running" else "Stopped"}")
                Spacer(modifier = Modifier.height(8.dp))
                Text(text = "Position: ${uiState.sCm} cm")
                Text(text = "Velocity: ${uiState.vCms} cm/s")

                Spacer(modifier = Modifier.height(32.dp))

                // Control button
                Button(
                    onClick = {
                        if (uiState.isRunning) {
                            viewModel.stopDetection()
                        } else {
                            viewModel.startDetection()
                        }
                    }
                ) {
                    Text(if (uiState.isRunning) "Stop Detection" else "Start Detection")
                }

                Spacer(modifier = Modifier.height(32.dp))

                // Event log
                if (events.isNotEmpty()) {
                    Text(text = "Recent Events:")
                    Spacer(modifier = Modifier.height(8.dp))
                    LazyColumn(
                        modifier = Modifier.weight(1f),
                        verticalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        items(events.take(20)) { event ->
                            EventItem(event)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun EventItem(event: PipelineEvent) {
    Row(
        modifier = Modifier.padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        when (event) {
            is PipelineEvent.Arrival -> {
                Text(text = "Arrival at stop ${event.stopIndex} (p=${event.probability})")
            }
            is PipelineEvent.Departure -> {
                Text(text = "Departure from stop ${event.stopIndex} (${event.dwellTimeS}s)")
            }
            is PipelineEvent.PositionUpdate -> {
                Text(text = "Pos: ${event.sCm} cm, Vel: ${event.vCms} cm/s")
            }
        }
    }
}
