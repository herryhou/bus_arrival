package com.busarrival.app.presentation.ui.detection

import androidx.compose.animation.core.*
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.compose.viewModel
import com.busarrival.app.presentation.ui.detection.components.EventToastHost
import com.busarrival.app.presentation.ui.detection.components.MapView
import com.busarrival.app.presentation.ui.detection.components.StatusPanel
import com.busarrival.app.presentation.viewmodel.DetectionViewModel
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.rememberMultiplePermissionsState

private class DetectionViewModelFactory(private val application: android.app.Application) :
        ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return DetectionViewModel(application) as T
    }
}

@OptIn(ExperimentalPermissionsApi::class)
@Composable
fun DetectionScreen(
        viewModel: DetectionViewModel =
                viewModel(
                        factory =
                                DetectionViewModelFactory(
                                        LocalContext.current.applicationContext as
                                                android.app.Application
                                )
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

    val locationPermissions =
            rememberMultiplePermissionsState(
                    permissions =
                            listOf(
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
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Glassmorphic background with proper layout
    Box(modifier = Modifier.fillMaxWidth().fillMaxHeight()) {
        if (!locationPermissions.allPermissionsGranted) {
            PermissionRequestContent(
                    onRequest = { locationPermissions.launchMultiplePermissionRequest() }
            )
        } else if (activeRoute == null) {
            NoRouteContent()
        } else {
            Column(
                    modifier =
                            Modifier.fillMaxWidth()
                                    .fillMaxHeight()
                                    .onSizeChanged {
                                        android.util.Log.d(
                                                "DEBUG",
                                                "Column size: ${it.width}x${it.height}"
                                        )
                                    }
                                    .drawBehind {
                                        // Purple glow (top-left)
                                        drawCircle(
                                                color = Color(0xFF6C5CE7).copy(alpha = 0.4f),
                                                radius = 300.dp.toPx() / 2,
                                                center =
                                                        androidx.compose.ui.geometry.Offset(
                                                                x = -80.dp.toPx(),
                                                                y = -100.dp.toPx()
                                                        )
                                        )
                                        // Teal glow (bottom-right)
                                        drawCircle(
                                                color = Color(0xFF00CEC9).copy(alpha = 0.3f),
                                                radius = 250.dp.toPx() / 2,
                                                center =
                                                        androidx.compose.ui.geometry.Offset(
                                                                x = size.width + 80.dp.toPx(),
                                                                y = size.height + 100.dp.toPx()
                                                        )
                                        )
                                    }
            ) {
                MapView(
                        routeData = activeRoute,
                        currentSCm = uiState.sCm,
                        replayState = replayState,
                        viewModel = viewModel,
                        gpsLat = uiState.gpsLat,
                        gpsLon = uiState.gpsLon,
                        gpsBearing = uiState.gpsBearing,
                        modifier =
                                Modifier.weight(0.6f)
                                        .onSizeChanged {
                                            android.util.Log.d(
                                                    "DEBUG",
                                                    "MapView size: ${it.width}x${it.height}"
                                            )
                                        }
                                        .border(BorderStroke(4.dp, Color.Cyan))
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
                        modifier =
                                Modifier.weight(0.4f).onSizeChanged {
                                    android.util.Log.d(
                                            "DEBUG",
                                            "StatusPanel size: ${it.width}x${it.height}"
                                    )
                                }
                )
            }
        }

        EventToastHost(hint = eventHints, modifier = Modifier.align(Alignment.TopCenter))

        // Error snackbar overlay
        uiState.error?.let { error ->
            ErrorSnackbar(error = error, onDismiss = { viewModel.clearError() })
        }
    }
}

@Composable
private fun PermissionRequestContent(onRequest: () -> Unit) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        GlassCard(modifier = Modifier.padding(horizontal = 32.dp)) {
            Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                    modifier = Modifier.padding(32.dp)
            ) {
                Icon(
                        imageVector = Icons.Default.LocationOn,
                        contentDescription = null,
                        modifier = Modifier.size(64.dp),
                        tint = Color(0xFF6C5CE7)
                )

                Text(
                        text = "Location Permission Required",
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.Bold,
                        color = Color.White
                )

                Text(
                        text =
                                "The app needs location access to detect bus arrivals along the route.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = Color.White.copy(alpha = 0.7f)
                )

                GlowingButton(
                        onClick = onRequest,
                        text = "Grant Permission",
                        icon = Icons.Default.Check
                )
            }
        }
    }
}

@Composable
private fun NoRouteContent() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            val infiniteTransition = rememberInfiniteTransition(label = "pulse")
            val scale by
                    infiniteTransition.animateFloat(
                            initialValue = 1f,
                            targetValue = 1.1f,
                            animationSpec =
                                    infiniteRepeatable(
                                            animation = tween(1500, easing = FastOutSlowInEasing),
                                            repeatMode = RepeatMode.Reverse
                                    ),
                            label = "pulse"
                    )

            Icon(
                    imageVector = Icons.Default.Place,
                    contentDescription = null,
                    modifier =
                            Modifier.size(80.dp).graphicsLayer {
                                scaleX = scale
                                scaleY = scale
                            },
                    tint = Color(0xFF6C5CE7).copy(alpha = 0.8f)
            )

            Text(
                    text = "No Route Loaded",
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White
            )

            Text(
                    text = "Please load a route in the Configuration tab first.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = Color.White.copy(alpha = 0.7f)
            )
        }
    }
}

@Composable
private fun ErrorSnackbar(error: String, onDismiss: () -> Unit, modifier: Modifier = Modifier) {
    Surface(
            modifier = modifier.padding(16.dp).fillMaxWidth().heightIn(min = 56.dp),
            shape = RoundedCornerShape(16.dp),
            color = Color(0xFF1A1A2E).copy(alpha = 0.95f),
            shadowElevation = 8.dp
    ) {
        Row(
                modifier = Modifier.fillMaxWidth().padding(16.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
        ) {
            Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.weight(1f)
            ) {
                Icon(
                        imageVector = Icons.Default.Warning,
                        contentDescription = null,
                        tint = Color(0xFFFF6B6B),
                        modifier = Modifier.size(24.dp)
                )
                Text(
                        text = error,
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color.White.copy(alpha = 0.9f)
                )
            }
            TextButton(onClick = onDismiss) {
                Text(
                        "Dismiss",
                        color = Color.White.copy(alpha = 0.7f),
                        fontWeight = FontWeight.Medium
                )
            }
        }
    }
}

// Glassmorphic card component
@Composable
private fun GlassCard(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
            modifier =
                    modifier.fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(
                                    Brush.verticalGradient(
                                            colors =
                                                    listOf(
                                                            Color.White.copy(alpha = 0.08f),
                                                            Color.White.copy(alpha = 0.03f)
                                                    )
                                    )
                            )
                            .padding(20.dp),
            content = content
    )
}

// Glowing button component
@Composable
private fun GlowingButton(
        onClick: () -> Unit,
        text: String,
        icon: androidx.compose.ui.graphics.vector.ImageVector
) {
    Button(
            onClick = onClick,
            modifier = Modifier.fillMaxWidth().height(56.dp),
            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF6C5CE7)),
            shape = RoundedCornerShape(16.dp)
    ) {
        Icon(imageVector = icon, contentDescription = null)
        Spacer(modifier = Modifier.width(8.dp))
        Text(text, fontWeight = FontWeight.Bold, color = Color.White)
    }
}
