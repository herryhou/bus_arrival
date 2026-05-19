package com.busarrival.app.presentation.ui.history

import android.app.Application
import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.Box
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.compose.ui.state.ToggleableState
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

@OptIn(ExperimentalMaterialApi::class)
@Composable
fun HistoryScreen(
    viewModel: HistoryViewModel = viewModel(
        factory = HistoryViewModelFactory(LocalContext.current.applicationContext as Application)
    )
) {
    val uiState by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    val pullRefreshState = rememberPullRefreshState(
        refreshing = uiState.isLoading,
        onRefresh = viewModel::refresh
    )
    val totalLogs = uiState.logs.size
    val allSelected = totalLogs > 0 && uiState.selectedCount == totalLogs
    val selectionLabel =
        when {
            totalLogs == 0 -> "No logs"
            uiState.selectedCount == 0 -> "Select all"
            allSelected -> "Clear selection"
            else -> "${uiState.selectedCount} selected"
        }
    val selectionState =
        when {
            uiState.selectedCount == 0 -> ToggleableState.Off
            allSelected -> ToggleableState.On
            else -> ToggleableState.Indeterminate
        }

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

            BulkActionBar(
                selectionState = selectionState,
                selectionLabel = selectionLabel,
                hasLogs = totalLogs > 0,
                canDelete = uiState.canDeleteSelected,
                onToggleSelection = {
                    if (allSelected) {
                        viewModel.clearSelection()
                    } else {
                        viewModel.selectAll()
                    }
                },
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

        Box(modifier = Modifier.fillMaxSize()) {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .pullRefresh(pullRefreshState),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                when {
                    uiState.isLoading -> item {
                        LoadingContent(modifier = Modifier.fillMaxSize())
                    }

                    uiState.logs.isEmpty() -> item {
                        EmptyLogsContent(modifier = Modifier.fillMaxSize())
                    }

                    else -> items(
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

            PullRefreshIndicator(
                refreshing = uiState.isLoading,
                state = pullRefreshState,
                modifier = Modifier.align(Alignment.TopCenter)
            )
        }
    }
}

@Composable
private fun BulkActionBar(
    selectionState: ToggleableState,
    selectionLabel: String,
    hasLogs: Boolean,
    canDelete: Boolean,
    onToggleSelection: () -> Unit,
    onShare: () -> Unit,
    onDelete: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        TriStateCheckbox(
            state = selectionState,
            onClick = onToggleSelection,
            enabled = hasLogs
        )

        Text(
            text = selectionLabel,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f)
        )

        IconButton(
            onClick = onShare,
            enabled = selectionState != ToggleableState.Off
        ) {
            Icon(
                imageVector = Icons.Default.Share,
                contentDescription = "Share selected logs"
            )
        }

        IconButton(
            onClick = onDelete,
            enabled = canDelete
        ) {
            Icon(
                imageVector = Icons.Default.Delete,
                contentDescription = "Delete selected logs"
            )
        }
    }
}

@Composable
private fun LoadingContent(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier,
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
private fun EmptyLogsContent(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.padding(horizontal = 24.dp),
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            text = "No GPS logs found",
            style = MaterialTheme.typography.titleLarge
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Start recording to create logs, then pull down to refresh.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
