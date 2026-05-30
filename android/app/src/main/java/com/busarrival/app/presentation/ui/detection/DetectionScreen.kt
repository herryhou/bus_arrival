package com.busarrival.app.presentation.ui.detection

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.busarrival.app.presentation.ui.detection.components.MapView
import com.busarrival.app.presentation.ui.detection.components.StatusPanel
import com.busarrival.app.presentation.ui.detection.components.TimelineScrubber
import com.busarrival.app.presentation.ui.detection.components.EventToastHost
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.rememberMultiplePermissionsState

private class DetectionViewModelFactory(
    private val application: android.app.Application
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return DetectionViewModel(application) as T
    }
}

@OptIn(ExperimentalPermissionsApi::class)
@Composable
fun DetectionScreen(
    viewModel: DetectionViewModel = viewModel(
        factory = DetectionViewModelFactory(LocalContext.current.applicationContext as android.app.Application)
    )
) {
    val uiState by viewModel.uiState.collectAsState()
    val events by viewModel.events.collectAsState()
    val activeRoute by viewModel.activeRoute.collectAsState()
    val activeRouteMetadata by viewModel.activeRouteMetadata.collectAsState()
    val replayState by viewModel.replayState.collectAsState()
    val gpsLoggingEnabled by viewModel.gpsLoggingEnabled.collectAsState()
    val gpsFixState by viewModel.gpsFixState.collectAsState()
    val eventHints by viewModel.eventHints.collectAsState()

    LaunchedEffect(Unit) {
        viewModel.refreshMapLabelZoomBias()
        viewModel.refreshLastGpsLogReference()
        viewModel.consumePendingGpsLogSimulation()
    }

    val locationPermissions = rememberMultiplePermissionsState(
        permissions = listOf(
            android.Manifest.permission.ACCESS_FINE_LOCATION,
            android.Manifest.permission.ACCESS_COARSE_LOCATION
        )
    )

    // Reload active route when screen becomes visible
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                viewModel.loadActiveRoute()
                viewModel.consumePendingGpsLogSimulation()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
        }
    }

    if (!locationPermissions.allPermissionsGranted) {
        PermissionRequestContent(onRequest = { locationPermissions.launchMultiplePermissionRequest() })
    } else if (activeRoute == null) {
        NoRouteContent()
    } else {
        Box(modifier = Modifier.fillMaxSize()) {
            Column(modifier = Modifier.fillMaxSize()) {
                MapView(
                    routeData = activeRoute,
                    currentSCm = uiState.sCm,
                    replayState = replayState,
                    viewModel = viewModel,
                    gpsLat = uiState.gpsLat,
                    gpsLon = uiState.gpsLon,
                    gpsBearing = uiState.gpsBearing,
                    modifier = Modifier.weight(0.6f)
                )

                StatusPanel(
                    uiState = uiState,
                    events = events,
                    routeName = activeRouteMetadata?.name,
                    gpsLoggingEnabled = gpsLoggingEnabled,
                    gpsFixState = gpsFixState,
                    replayState = replayState,
                    onStartStop = {
                        if (uiState.isRunning) {
                            viewModel.stopDetection()
                        } else {
                            viewModel.startDetection()
                        }
                    },
                    onToggleGpsLogging = { viewModel.toggleGpsLogging() },
                    onPlayPause = { viewModel.playPause() },
                    onSeek = { viewModel.seekTo(it) },
                    onSpeedChange = { viewModel.setPlaybackSpeed(it) },
                    modifier = Modifier
                        .weight(0.4f)
                        .fillMaxWidth()
                )
            }

            // Event hints overlay (outside Column to avoid blocking touches)
            EventToastHost(
                hint = eventHints,
                modifier = Modifier.align(Alignment.TopCenter)
            )
        }
    }

    uiState.error?.let { error ->
        ErrorSnackbar(
            error = error,
            onDismiss = { viewModel.clearError() }
        )
    }
}

@Composable
private fun PermissionRequestContent(onRequest: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier.padding(24.dp).widthIn(max = 420.dp),
            shape = RoundedCornerShape(8.dp),
            color = MaterialTheme.colorScheme.surface,
            tonalElevation = 1.dp
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp),
                modifier = Modifier.padding(28.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.LocationOn,
                    contentDescription = null,
                    modifier = Modifier.size(64.dp),
                    tint = MaterialTheme.colorScheme.primary
                )

                Text(
                    text = "Location Permission Required",
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold
                )

                Text(
                    text = "The app needs location access to detect bus arrivals along the route.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Button(
                    onClick = onRequest,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)
                ) {
                    Icon(
                        imageVector = Icons.Default.Check,
                        contentDescription = null,
                        modifier = Modifier.size(20.dp)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Grant Permission")
                }
            }
        }
    }
}

@Composable
private fun NoRouteContent() {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier.padding(24.dp).widthIn(max = 420.dp),
            shape = RoundedCornerShape(8.dp),
            color = MaterialTheme.colorScheme.surface,
            tonalElevation = 1.dp
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp),
                modifier = Modifier.padding(28.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.Place,
                    contentDescription = null,
                    modifier = Modifier.size(64.dp),
                    tint = MaterialTheme.colorScheme.tertiary
                )

                Text(
                    text = "No Route Loaded",
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold
                )

                Text(
                    text = "Please load a route in the Configuration tab first.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Button(
                    onClick = { /* Navigate to config */ },
                    modifier = Modifier.heightIn(min = 48.dp)
                ) {
                    Text("Go to Configuration")
                }
            }
        }
    }
}

@Composable
private fun ErrorSnackbar(
    error: String,
    onDismiss: () -> Unit
) {
    Snackbar(
        modifier = Modifier.padding(16.dp),
        action = {
            TextButton(onClick = onDismiss) {
                Text("Dismiss")
            }
        }
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(
                imageVector = Icons.Default.Warning,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.error,
                modifier = Modifier.size(20.dp)
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(error)
        }
    }
}
