package com.busarrival.app.presentation.ui.history

import android.app.Application
import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.compose.viewModel
import com.busarrival.app.presentation.ui.history.components.GpsLogRow
import com.busarrival.app.presentation.viewmodel.HistoryViewModel
import com.busarrival.app.service.GpsLogArchive

private class HistoryViewModelFactory(
    private val application: Application
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return HistoryViewModel(application) as T
    }
}

@Composable
fun HistoryScreen(
    viewModel: HistoryViewModel = viewModel(
        factory = HistoryViewModelFactory(LocalContext.current.applicationContext as Application)
    )
) {
    val uiState by viewModel.uiState.collectAsState()
    val context = LocalContext.current

    Column(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp)
        ) {
            Text(
                text = "GPS Logs",
                style = MaterialTheme.typography.headlineSmall
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = "Manage the logs from the active storage backend.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            if (uiState.error != null) {
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = uiState.error.orEmpty(),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium
                )
            }

            Spacer(modifier = Modifier.height(12.dp))

            ActionBar(
                selectedCount = uiState.selectedCount,
                hasLogs = uiState.logs.isNotEmpty(),
                canDelete = uiState.canDeleteSelected,
                onSelectAll = viewModel::selectAll,
                onClearSelection = viewModel::clearSelection,
                onRefresh = viewModel::refresh,
                onShare = {
                    viewModel.shareSelected()?.let { zipFile ->
                        val shareIntent =
                            Intent.createChooser(
                                GpsLogArchive.shareZip(context, zipFile),
                                "Share GPS logs"
                            ).apply {
                                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                            }
                        context.startActivity(shareIntent)
                    }
                },
                onDelete = viewModel::deleteSelected
            )
        }

        HorizontalDivider()

        when {
            uiState.isLoading -> LoadingContent()
            uiState.logs.isEmpty() -> EmptyLogsContent(onRefresh = viewModel::refresh)
            else -> {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    items(
                        items = uiState.logs,
                        key = { it.reference }
                    ) { item ->
                        GpsLogRow(
                            item = item,
                            onToggle = viewModel::toggleSelection
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ActionBar(
    selectedCount: Int,
    hasLogs: Boolean,
    canDelete: Boolean,
    onSelectAll: () -> Unit,
    onClearSelection: () -> Unit,
    onRefresh: () -> Unit,
    onShare: () -> Unit,
    onDelete: () -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Button(onClick = onSelectAll, enabled = hasLogs && selectedCount == 0) {
            Text("Select all")
        }

        OutlinedButton(onClick = onClearSelection, enabled = selectedCount > 0) {
            Text("Clear")
        }

        OutlinedButton(onClick = onRefresh) { Text("Refresh") }

        Button(onClick = onShare, enabled = selectedCount > 0) {
            Text("Share $selectedCount")
        }

        Button(onClick = onDelete, enabled = canDelete) { Text("Delete") }
    }
}

@Composable
private fun LoadingContent() {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            text = "Loading GPS logs...",
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.padding(24.dp)
        )
    }
}

@Composable
private fun EmptyLogsContent(onRefresh: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            text = "No GPS logs found",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Start recording to create logs, then refresh this screen.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(modifier = Modifier.height(16.dp))
        OutlinedButton(
            onClick = onRefresh,
            modifier = Modifier.padding(horizontal = 24.dp)
        ) {
            Text("Refresh")
        }
    }
}
