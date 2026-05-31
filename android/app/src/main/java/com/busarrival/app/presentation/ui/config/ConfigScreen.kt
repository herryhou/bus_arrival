package com.busarrival.app.presentation.ui.config

import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.busarrival.app.domain.model.RouteMetadata
import com.busarrival.app.presentation.ui.config.components.GlassCard
import com.busarrival.app.presentation.ui.config.components.GlowingButton
import com.busarrival.app.presentation.ui.config.components.ParameterSlider
import com.busarrival.app.presentation.ui.config.components.RouteCard
import com.busarrival.app.presentation.viewmodel.ConfigViewModel
import kotlinx.coroutines.delay

private class ConfigViewModelFactory(
    private val application: android.app.Application
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return ConfigViewModel(application) as T
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConfigScreen(
    viewModel: ConfigViewModel = viewModel(
        factory = ConfigViewModelFactory(LocalContext.current.applicationContext as android.app.Application)
    )
) {
    val uiState by viewModel.uiState.collectAsState()
    var showDeleteDialog by remember { mutableStateOf<RouteMetadata?>(null) }
    var showParametersSheet by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val sheetState = rememberModalBottomSheetState(
        skipPartiallyExpanded = true
    )

    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        uri?.let {
            val fileName = getFileNameFromUri(context, it)
            viewModel.addRoute(it, fileName)
        }
    }

    // Animated background gradient
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.linearGradient(
                    colors = listOf(
                        Color(0xFF1A1A2E).copy(alpha = 0.95f),
                        Color(0xFF16213E).copy(alpha = 0.9f),
                        Color(0xFF0F0F1A)
                    )
                )
            )
    ) {
        // Glowing orbs
        Box(
            modifier = Modifier
                .fillMaxSize()
                .blur(100.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(300.dp)
                    .background(
                        Brush.radialGradient(
                            colors = listOf(
                                Color(0xFF6C5CE7).copy(alpha = 0.4f),
                                Color.Transparent
                            )
                        )
                    )
                    .align(Alignment.TopStart)
                    .offset(x = (-80).dp, y = (-100).dp)
            )
            Box(
                modifier = Modifier
                    .size(250.dp)
                    .background(
                        Brush.radialGradient(
                            colors = listOf(
                                Color(0xFF00CEC9).copy(alpha = 0.3f),
                                Color.Transparent
                            )
                        )
                    )
                    .align(Alignment.BottomEnd)
                    .offset(x = 80.dp, y = 100.dp)
            )
        }

        // Main content
        if (uiState.isLoading) {
            LoadingContent()
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal).asPaddingValues(),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                item {
                    Spacer(modifier = Modifier.height(24.dp))

                    // Header
                    Column(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Text(
                            text = "Setup",
                            style = MaterialTheme.typography.displaySmall,
                            fontWeight = FontWeight.Bold,
                            color = Color.White,
                            letterSpacing = (-0.02).em
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = "Configure routes & detection",
                            style = MaterialTheme.typography.bodyLarge,
                            color = Color.White.copy(alpha = 0.7f)
                        )
                    }

                    Spacer(modifier = Modifier.height(24.dp))
                }

                if (uiState.routes.isEmpty()) {
                    item {
                        EmptyRoutesContent(onAddRoute = {
                            filePickerLauncher.launch(arrayOf("*/*"))
                        })
                    }
                } else {
                    // Active route highlight
                    item {
                        val activeRoute = uiState.routes.firstOrNull {
                            it.uuid == uiState.activeRouteId
                        }
                        if (activeRoute != null) {
                            ActiveRouteCard(
                                metadata = activeRoute,
                                onClick = { viewModel.setActiveRoute(activeRoute.uuid) }
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                        }
                    }

                    // Route list
                    item {
                        Text(
                            text = "Routes",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = Color.White.copy(alpha = 0.9f),
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 20.dp)
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                    }

                    items(
                        items = uiState.routes,
                        key = { it.uuid }
                    ) { metadata ->
                        val isActive = metadata.uuid == uiState.activeRouteId
                        RouteCard(
                            metadata = metadata,
                            isActive = isActive,
                            onClick = { viewModel.setActiveRoute(metadata.uuid) },
                            onDelete = { showDeleteDialog = metadata }
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                    }

                    // Quick actions
                    item {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 20.dp),
                            horizontalArrangement = Arrangement.spacedBy(12.dp)
                        ) {
                            GlassCard(
                                modifier = Modifier.weight(1f),
                                onClick = { showParametersSheet = true }
                            ) {
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    modifier = Modifier.padding(20.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Rounded.Tune,
                                        contentDescription = null,
                                        tint = Color(0xFF00CEC9),
                                        modifier = Modifier.size(28.dp)
                                    )
                                    Spacer(modifier = Modifier.height(8.dp))
                                    Text(
                                        text = "Parameters",
                                        style = MaterialTheme.typography.labelLarge,
                                        color = Color.White
                                    )
                                }
                            }

                            GlassCard(
                                modifier = Modifier.weight(1f),
                                onClick = { filePickerLauncher.launch(arrayOf("*/*")) }
                            ) {
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    modifier = Modifier.padding(20.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Rounded.Add,
                                        contentDescription = null,
                                        tint = Color(0xFF6C5CE7),
                                        modifier = Modifier.size(28.dp)
                                    )
                                    Spacer(modifier = Modifier.height(8.dp))
                                    Text(
                                        text = "Add Route",
                                        style = MaterialTheme.typography.labelLarge,
                                        color = Color.White
                                    )
                                }
                            }
                        }
                    }

                    item { Spacer(modifier = Modifier.height(100.dp)) }
                }
            }
        }
    }

    // Parameters bottom sheet
    if (showParametersSheet) {
        ModalBottomSheet(
            onDismissRequest = { showParametersSheet = false },
            sheetState = sheetState,
            containerColor = Color.Transparent,
            scrimColor = Color.Black.copy(alpha = 0.4f),
            dragHandle = null
        ) {
            ParametersSheetContent(
                parameters = uiState.parameters,
                mapLabelBias = uiState.mapLabelZoomBias,
                onParameterChange = { param, value ->
                    when (param) {
                        "distanceWeight" -> viewModel.updateParameter(distanceWeight = value)
                        "speedWeight" -> viewModel.updateParameter(speedWeight = value)
                        "progressErrorWeight" -> viewModel.updateParameter(progressErrorWeight = value)
                        "dwellTimeWeight" -> viewModel.updateParameter(dwellTimeWeight = value)
                        "corridorSize" -> viewModel.updateParameter(corridorSize = value)
                    }
                },
                onMapLabelBiasChange = { viewModel.setMapLabelZoomBias(it) },
                onClose = { showParametersSheet = false }
            )
        }
    }

    // Delete confirmation
    showDeleteDialog?.let { metadata ->
        DeleteConfirmationDialog(
            routeName = metadata.name,
            onConfirm = {
                viewModel.deleteRoute(metadata)
                showDeleteDialog = null
            },
            onDismiss = { showDeleteDialog = null }
        )
    }

    // Error dialog
    uiState.error?.let { error ->
        ErrorDialog(
            error = error,
            onDismiss = { viewModel.clearError() }
        )
    }
}

@Composable
private fun ActiveRouteCard(
    metadata: RouteMetadata,
    onClick: () -> Unit
) {
    GlassCard(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp),
        onClick = onClick
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(20.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = "Active",
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.Medium,
                        color = Color(0xFF00CEC9)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Box(
                        modifier = Modifier
                            .size(6.dp)
                            .clip(CircleShape)
                            .background(Color(0xFF00CEC9))
                    )
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = metadata.name,
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                    color = Color.White
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "${metadata.stopCount} stops",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color.White.copy(alpha = 0.6f)
                )
            }
        }
    }
}

@Composable
private fun ParametersSheetContent(
    parameters: com.busarrival.app.domain.model.DetectionParameters,
    mapLabelBias: Int,
    onParameterChange: (String, Int) -> Unit,
    onMapLabelBiasChange: (Int) -> Unit,
    onClose: () -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        Color(0xFF1A1A2E).copy(alpha = 0.95f),
                        Color(0xFF0F0F1A)
                    )
                ),
                shape = RoundedCornerShape(28.dp, 28.dp, 0.dp, 0.dp)
            )
            .padding(24.dp)
    ) {
        Column {
            // Handle bar
            Box(
                modifier = Modifier
                    .width(40.dp)
                    .height(4.dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(Color.White.copy(alpha = 0.3f))
                    .align(Alignment.CenterHorizontally)
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Header
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = "Detection Parameters",
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.Bold,
                        color = Color.White
                    )
                    Text(
                        text = "Fine-tune arrival detection",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color.White.copy(alpha = 0.6f)
                    )
                }
                IconButton(onClick = onClose) {
                    Icon(
                        imageVector = Icons.Rounded.Close,
                        contentDescription = "Close",
                        tint = Color.White.copy(alpha = 0.7f)
                    )
                }
            }

            Spacer(modifier = Modifier.height(24.dp))

            // Map label bias
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Map Labels",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = Color.White
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(
                        0 to "Small",
                        1 to "Medium",
                        2 to "Large"
                    ).forEach { (bias, label) ->
                        FilterChip(
                            selected = mapLabelBias == bias,
                            onClick = { onMapLabelBiasChange(bias) },
                            label = { Text(label) },
                            colors = FilterChipDefaults.filterChipColors(
                                selectedContainerColor = Color(0xFF6C5CE7),
                                selectedLabelColor = Color.White
                            ),
                            border = null
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(24.dp))

            // Parameters
            ParameterSlider(
                label = "Distance Weight",
                value = parameters.distanceWeight,
                onValueChange = { onParameterChange("distanceWeight", it) }
            )

            ParameterSlider(
                label = "Speed Weight",
                value = parameters.speedWeight,
                onValueChange = { onParameterChange("speedWeight", it) }
            )

            ParameterSlider(
                label = "Progress Error Weight",
                value = parameters.progressErrorWeight,
                onValueChange = { onParameterChange("progressErrorWeight", it) }
            )

            ParameterSlider(
                label = "Dwell Time Weight",
                value = parameters.dwellTimeWeight,
                onValueChange = { onParameterChange("dwellTimeWeight", it) }
            )

            ParameterSlider(
                label = "Corridor Size",
                value = parameters.corridorSize,
                onValueChange = { onParameterChange("corridorSize", it) },
                valueRange = -80..40,
                suffix = "m"
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Auto-save indicator
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.Center
            ) {
                Text(
                    text = "Changes auto-saved",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color(0xFF00CEC9)
                )
            }

            Spacer(modifier = Modifier.height(32.dp))
        }
    }
}

@Composable
private fun LoadingContent() {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(60.dp)
                    .blur(20.dp)
                    .background(
                        Brush.radialGradient(
                            colors = listOf(
                                Color(0xFF6C5CE7).copy(alpha = 0.6f),
                                Color.Transparent
                            )
                        )
                    )
            )

            CircularProgressIndicator(
                modifier = Modifier.size(48.dp),
                color = Color(0xFF6C5CE7),
                strokeWidth = 3.dp
            )
            Text(
                text = "Loading routes...",
                style = MaterialTheme.typography.bodyLarge,
                color = Color.White.copy(alpha = 0.7f)
            )
        }
    }
}

@Composable
private fun EmptyRoutesContent(onAddRoute: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Spacer(modifier = Modifier.height(40.dp))

        // Animated icon
        val infiniteTransition = rememberInfiniteTransition(label = "pulse")
        val scale by infiniteTransition.animateFloat(
            initialValue = 1f,
            targetValue = 1.1f,
            animationSpec = infiniteRepeatable(
                animation = tween(1500, easing = FastOutSlowInEasing),
                repeatMode = RepeatMode.Reverse
            ),
            label = "pulse"
        )

        Icon(
            imageVector = Icons.Rounded.Add,
            contentDescription = null,
            modifier = Modifier
                .size(80.dp)
                .scale(scale),
            tint = Color(0xFF6C5CE7).copy(alpha = 0.8f)
        )

        Spacer(modifier = Modifier.height(24.dp))

        Text(
            text = "No routes yet",
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.Bold,
            color = Color.White,
            textAlign = TextAlign.Center
        )

        Spacer(modifier = Modifier.height(12.dp))

        Text(
            text = "Add your first route file to start detecting arrivals",
            style = MaterialTheme.typography.bodyLarge,
            color = Color.White.copy(alpha = 0.6f),
            textAlign = TextAlign.Center
        )

        Spacer(modifier = Modifier.height(32.dp))

        GlowingButton(
            onClick = onAddRoute,
            text = "Add Route",
            icon = Icons.Rounded.Add
        )
    }
}

@Composable
private fun DeleteConfirmationDialog(
    routeName: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Color(0xFF1A1A2E),
        title = {
            Text(
                text = "Delete Route?",
                fontWeight = FontWeight.Bold,
                color = Color.White
            )
        },
        text = {
            Text(
                "Are you sure you want to delete \"$routeName\"? This action cannot be undone.",
                color = Color.White.copy(alpha = 0.7f)
            )
        },
        confirmButton = {
            TextButton(
                onClick = onConfirm,
                colors = ButtonDefaults.textButtonColors(
                    contentColor = Color(0xFFFF6B6B)
                )
            ) {
                Text("Delete", fontWeight = FontWeight.Bold)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = Color.White.copy(alpha = 0.7f))
            }
        }
    )
}

@Composable
private fun ErrorDialog(
    error: String,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Color(0xFF1A1A2E),
        title = {
            Text(
                text = "Error",
                fontWeight = FontWeight.Bold,
                color = Color.White
            )
        },
        text = {
            Text(
                error,
                color = Color.White.copy(alpha = 0.7f)
            )
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("OK", color = Color(0xFF6C5CE7))
            }
        }
    )
}

private fun getFileNameFromUri(context: android.content.Context, uri: Uri): String {
    var fileName: String? = null
    context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) {
            val nameIndex = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
            if (nameIndex >= 0) {
                fileName = cursor.getString(nameIndex)
            }
        }
    }

    if (fileName.isNullOrBlank()) {
        fileName = uri.lastPathSegment
    }

    fileName = fileName?.removeSuffix(".bin")
    return fileName ?: "Route ${System.currentTimeMillis()}"
}
