package com.busarrival.app.presentation.ui.config

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.busarrival.app.domain.model.RouteMetadata
import com.busarrival.app.presentation.ui.config.components.ParameterSlider
import com.busarrival.app.presentation.ui.config.components.RouteListItem
import com.busarrival.app.presentation.viewmodel.ConfigViewModel

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
    var parametersExpanded by remember { mutableStateOf(false) }

    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        uri?.let {
            viewModel.addRoute(it, "Route ${System.currentTimeMillis()}")
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Configuration") }
            )
        }
    ) { padding ->
        Box(modifier = Modifier.fillMaxSize()) {
            if (uiState.isLoading) {
                LoadingContent()
            } else {
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp)
                ) {
                    item {
                        Text(
                            text = "Routes",
                            style = MaterialTheme.typography.titleMedium
                        )
                    }

                    if (uiState.routes.isEmpty()) {
                        item {
                            EmptyRoutesContent()
                        }
                    } else {
                        items(
                            items = uiState.routes,
                            key = { it.uuid }
                        ) { metadata ->
                            val isActive = metadata.uuid == uiState.activeRouteId
                            RouteListItem(
                                metadata = metadata,
                                isActive = isActive,
                                onClick = { viewModel.setActiveRoute(metadata.uuid) },
                                onDelete = { showDeleteDialog = metadata }
                            )
                        }
                    }

                    item {
                        Spacer(modifier = Modifier.height(16.dp))

                        Card(
                            modifier = Modifier.fillMaxWidth(),
                            elevation = CardDefaults.cardElevation(defaultElevation = 2.dp)
                        ) {
                            Column(modifier = Modifier.padding(16.dp)) {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Text(
                                        text = "Detection Parameters",
                                        style = MaterialTheme.typography.titleMedium
                                    )
                                    IconButton(onClick = { parametersExpanded = !parametersExpanded }) {
                                        Icon(
                                            imageVector = if (parametersExpanded) Icons.Default.KeyboardArrowUp else Icons.Default.KeyboardArrowDown,
                                            contentDescription = if (parametersExpanded) "Collapse" else "Expand"
                                        )
                                    }
                                }

                                AnimatedVisibility(
                                    visible = parametersExpanded,
                                    enter = expandVertically() + fadeIn(),
                                    exit = shrinkVertically() + fadeOut()
                                ) {
                                    Column {
                                        Spacer(modifier = Modifier.height(16.dp))

                                        ParameterSlider(
                                            label = "Distance Weight",
                                            value = uiState.parameters.distanceWeight,
                                            onValueChange = { viewModel.updateParameter(distanceWeight = it) }
                                        )

                                        ParameterSlider(
                                            label = "Speed Weight",
                                            value = uiState.parameters.speedWeight,
                                            onValueChange = { viewModel.updateParameter(speedWeight = it) }
                                        )

                                        ParameterSlider(
                                            label = "Progress Error Weight",
                                            value = uiState.parameters.progressErrorWeight,
                                            onValueChange = { viewModel.updateParameter(progressErrorWeight = it) }
                                        )

                                        ParameterSlider(
                                            label = "Dwell Time Weight",
                                            value = uiState.parameters.dwellTimeWeight,
                                            onValueChange = { viewModel.updateParameter(dwellTimeWeight = it) }
                                        )

                                        ParameterSlider(
                                            label = "Corridor Size",
                                            value = uiState.parameters.corridorSize,
                                            onValueChange = { viewModel.updateParameter(corridorSize = it) },
                                            valueRange = -80..40,
                                            suffix = "m"
                                        )

                                        Spacer(modifier = Modifier.height(16.dp))

                                        Button(
                                            onClick = { /* Parameters auto-save */ },
                                            modifier = Modifier.fillMaxWidth(),
                                            enabled = false
                                        ) {
                                            Text("Parameters Auto-Saved")
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            FloatingActionButton(
                onClick = {
                    filePickerLauncher.launch(arrayOf("*/*"))
                },
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(16.dp)
            ) {
                Icon(imageVector = Icons.Default.Add, contentDescription = "Add route")
            }
        }
    }

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

    uiState.error?.let { error ->
        ErrorDialog(
            error = error,
            onDismiss = { viewModel.clearError() }
        )
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
            CircularProgressIndicator(
                modifier = Modifier.size(48.dp)
            )
            Text(
                text = "Loading routes...",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun EmptyRoutesContent() {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Icon(
            imageVector = Icons.Default.Add,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "No routes loaded",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Tap + to add a route file",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
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
        title = { Text("Delete Route?") },
        text = {
            Text("Are you sure you want to delete \"$routeName\"? This action cannot be undone.")
        },
        confirmButton = {
            TextButton(
                onClick = onConfirm,
                colors = ButtonDefaults.textButtonColors(
                    contentColor = MaterialTheme.colorScheme.error
                )
            ) {
                Text("Delete", fontWeight = FontWeight.Bold)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
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
        title = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = Icons.Default.Add,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                    modifier = Modifier.size(24.dp)
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text("Error")
            }
        },
        text = { Text(error) },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("OK")
            }
        }
    )
}
