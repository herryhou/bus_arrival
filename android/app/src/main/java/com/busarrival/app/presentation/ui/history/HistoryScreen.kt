package com.busarrival.app.presentation.ui.history

import android.app.Application
import android.content.Intent
import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Sort
import androidx.compose.material3.*
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.compose.viewModel
import com.busarrival.app.presentation.ui.*
import com.busarrival.app.presentation.ui.history.components.GlassCard
import com.busarrival.app.presentation.ui.history.components.GlassLogItem
import com.busarrival.app.presentation.viewmodel.HistoryViewModel
import com.busarrival.app.service.GpsLogArchive
import kotlinx.coroutines.delay

private class HistoryViewModelFactory(
    private val application: Application
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return HistoryViewModel(application) as T
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterialApi::class)
@Composable
fun HistoryScreen(
    onSimulateLog: () -> Unit = {},
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
    val hasSelection = uiState.selectedCount > 0

    Box(modifier = Modifier.fillMaxSize()) {
        // Glassmorphic background
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.linearGradient(
                        colors = listOf(
                            Color(0xFF1A1A2E).copy(alpha = 0.55f),
                            Color(0xFF16213E).copy(alpha = 0.7f),
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
                        .size(280.dp)
                        .background(
                            Brush.radialGradient(
                                colors = listOf(
                                    Color(0xFF6C5CE7).copy(alpha = 0.35f),
                                    Color.Transparent
                                )
                            )
                        )
                        .align(Alignment.TopStart)
                        .offset(x = (-60).dp, y = (-80).dp)
                )
                Box(
                    modifier = Modifier
                        .size(220.dp)
                        .background(
                            Brush.radialGradient(
                                colors = listOf(
                                    AccentPrimary.copy(alpha = 0.25f),
                                    Color.Transparent
                                )
                            )
                        )
                        .align(Alignment.BottomEnd)
                        .offset(x = 70.dp, y = 90.dp)
                )
            }

            // Main content
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .pullRefresh(pullRefreshState),
                contentPadding = PaddingValues(
                    start = 20.dp,
                    end = 20.dp,
                    top = 200.dp,
                    bottom = 100.dp
                )
            ) {
                when {
                    uiState.isLoading && uiState.logs.isEmpty() -> item {
                        LoadingContent()
                    }

                    uiState.logs.isEmpty() -> item {
                        EmptyLogsContent()
                    }

                    else -> {
                        items(
                            items = uiState.logs,
                            key = { it.reference }
                        ) { item ->
                            GlassLogItem(
                                item = item,
                                onToggle = viewModel::toggleSelection,
                                onSimulate = { reference ->
                                    if (viewModel.requestSimulation(reference)) {
                                        onSimulateLog()
                                    }
                                }
                            )
                            Spacer(modifier = Modifier.height(12.dp))
                        }
                    }
                }
            }
        }

        // Header
        GlassHeader(
            totalLogs = totalLogs,
            selectedCount = uiState.selectedCount,
            hasSelection = hasSelection,
            allSelected = allSelected,
            onSortClick = { /* TODO: Implement sort */ },
            onSelectAll = { if (allSelected) viewModel.clearSelection() else viewModel.selectAll() },
            onShareSelected = {
                viewModel.shareSelected()?.let { zipFile ->
                    val shareIntent = Intent.createChooser(
                        GpsLogArchive.shareZip(context, zipFile),
                        "Share GPS logs"
                    ).apply {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    context.startActivity(shareIntent)
                }
            },
            onDeleteSelected = viewModel::deleteSelected,
            canDelete = uiState.canDeleteSelected
        )

        // Pull refresh indicator (glassmorphic)
        PullRefreshIndicator(
            refreshing = uiState.isLoading,
            state = pullRefreshState,
            modifier = Modifier.align(Alignment.TopCenter),
            backgroundColor = Color.Transparent,
            contentColor = Color(0xFF6C5CE7),
            scale = true
        )

        // Error banner
        val errorMessage = uiState.error
        if (errorMessage != null) {
            ErrorBanner(
                error = errorMessage,
                onDismiss = { viewModel.clearError() },
                modifier = Modifier.align(Alignment.TopCenter)
            )
        }
    }
}

@Composable
private fun GlassHeader(
    totalLogs: Int,
    selectedCount: Int,
    hasSelection: Boolean,
    allSelected: Boolean,
    onSortClick: () -> Unit,
    onSelectAll: () -> Unit,
    onShareSelected: () -> Unit,
    onDeleteSelected: () -> Unit,
    canDelete: Boolean
) {
    val animatedHeight by animateDpAsState(
        targetValue = 190.dp,
        animationSpec = spring(dampingRatio = 0.8f, stiffness = 300f),
        label = "headerHeight"
    )

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(animatedHeight)
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        Color(0xFF1A1A2E).copy(alpha = 0.92f),
                        Color(0xFF0F0F1A).copy(alpha = 0.96f)
                    )
                )
            )
            .border(
                BorderStroke(1.dp, Color.White.copy(alpha = 0.12f)),
                RoundedCornerShape(0.dp)
            )
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 16.dp)
        ) {
            // Title row
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.History,
                        contentDescription = null,
                        tint = Color(0xFF6C5CE7),
                        modifier = Modifier.size(28.dp)
                    )
                    Column {
                        Text(
                            text = "GPS Logs",
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.Bold,
                            color = Color.White
                        )
                        Text(
                            text = "$totalLogs logs",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Color.White.copy(alpha = 0.6f)
                        )
                    }
                }

                IconButton(
                    onClick = onSortClick,
                    modifier = Modifier.size(40.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Sort,
                        contentDescription = "Sort",
                        tint = Color.White.copy(alpha = 0.7f),
                        modifier = Modifier.size(22.dp)
                    )
                }
            }

            AnimatedVisibility(visible = hasSelection) {
                // Selection actions
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp)
                ) {
                    Column {
                        Spacer(modifier = Modifier.height(16.dp))

                        // Selection toolbar
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(16.dp))
                                .background(Color.White.copy(alpha = 0.06f))
                                .border(
                                    BorderStroke(1.dp, Color.White.copy(alpha = 0.08f)),
                                    RoundedCornerShape(16.dp)
                                )
                                .padding(horizontal = 16.dp, vertical = 16.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                                modifier = Modifier.weight(1f)
                            ) {
                                // Checkbox
                                Box(
                                    modifier = Modifier
                                        .size(24.dp)
                                        .clip(RoundedCornerShape(6.dp))
                                        .background(
                                            when {
                                                allSelected -> Color(0xFF6C5CE7)
                                                selectedCount > 0 -> Color(0xFF6C5CE7).copy(alpha = 0.6f)
                                                else -> Color.White.copy(alpha = 0.1f)
                                            }
                                        )
                                        .border(
                                            BorderStroke(
                                                1.5.dp,
                                                when {
                                                    allSelected -> Color(0xFF6C5CE7)
                                                    selectedCount > 0 -> Color(0xFF6C5CE7).copy(alpha = 0.8f)
                                                    else -> Color.White.copy(alpha = 0.3f)
                                                }
                                            ),
                                            RoundedCornerShape(6.dp)
                                        )
                                        .clip(RoundedCornerShape(6.dp))
                                        .clickable { onSelectAll() },
                                    contentAlignment = Alignment.Center
                                ) {
                                    if (allSelected) {
                                        Icon(
                                            imageVector = Icons.Default.Share,
                                            contentDescription = null,
                                            tint = Color.White,
                                            modifier = Modifier.size(14.dp)
                                        )
                                    } else if (selectedCount > 0) {
                                        Box(
                                            modifier = Modifier
                                                .size(10.dp)
                                                .background(Color.White, CircleShape)
                                        )
                                    }
                                }

                                Text(
                                    text = "$selectedCount selected",
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontWeight = FontWeight.Medium,
                                    color = Color.White.copy(alpha = 0.9f)
                                )
                            }

                            // Action buttons
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(8.dp)
                            ) {
                                // Share button
                                Box(
                                    modifier = Modifier
                                        .clip(RoundedCornerShape(12.dp))
                                        .background(Color(0xFF00CEC9).copy(alpha = 0.15f))
                                        .clickable { onShareSelected() }
                                        .padding(horizontal = 12.dp, vertical = 10.dp)
                                ) {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                                    ) {
                                        Icon(
                                            imageVector = Icons.Default.Share,
                                            contentDescription = "Share",
                                            tint = Color(0xFF00CEC9),
                                            modifier = Modifier.size(18.dp)
                                        )
                                        Text(
                                            text = "Share",
                                            style = MaterialTheme.typography.labelMedium,
                                            fontWeight = FontWeight.SemiBold,
                                            color = Color(0xFF00CEC9)
                                        )
                                    }
                                }

                                // Delete button
                                Box(
                                    modifier = Modifier
                                        .clip(RoundedCornerShape(12.dp))
                                        .background(
                                            if (canDelete)
                                                Color(0xFFFF6B6B).copy(alpha = 0.15f)
                                            else
                                                Color.White.copy(alpha = 0.05f)
                                        )
                                        .clickable(enabled = canDelete) { onDeleteSelected() }
                                        .padding(horizontal = 12.dp, vertical = 10.dp)
                                ) {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                                    ) {
                                        Icon(
                                            imageVector = Icons.Default.Delete,
                                            contentDescription = "Delete",
                                            tint = if (canDelete) Color(0xFFFF6B6B) else Color.White.copy(alpha = 0.3f),
                                            modifier = Modifier.size(18.dp)
                                        )
                                        Text(
                                            text = "Delete",
                                            style = MaterialTheme.typography.labelMedium,
                                            fontWeight = FontWeight.SemiBold,
                                            color = if (canDelete) Color(0xFFFF6B6B) else Color.White.copy(alpha = 0.3f)
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LoadingContent() {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 60.dp),
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
                text = "Loading GPS logs...",
                style = MaterialTheme.typography.bodyLarge,
                color = Color.White.copy(alpha = 0.7f)
            )
        }
    }
}

@Composable
private fun EmptyLogsContent() {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 60.dp, horizontal = 32.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Spacer(modifier = Modifier.height(20.dp))

        // Animated icon
        val infiniteTransition = rememberInfiniteTransition(label = "pulse")
        val scale by infiniteTransition.animateFloat(
            initialValue = 1f,
            targetValue = 1.08f,
            animationSpec = infiniteRepeatable(
                animation = tween(1500, easing = FastOutSlowInEasing),
                repeatMode = RepeatMode.Reverse
            ),
            label = "pulse"
        )

        Icon(
            imageVector = Icons.Rounded.History,
            contentDescription = null,
            modifier = Modifier
                .size(72.dp)
                .scale(scale),
            tint = Color(0xFF6C5CE7).copy(alpha = 0.7f)
        )

        Spacer(modifier = Modifier.height(20.dp))

        Text(
            text = "No GPS logs yet",
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.Bold,
            color = Color.White,
            textAlign = TextAlign.Center
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Start recording to create logs, then pull down to refresh",
            style = MaterialTheme.typography.bodyLarge,
            color = Color.White.copy(alpha = 0.6f),
            textAlign = TextAlign.Center
        )
    }
}

@Composable
private fun ErrorBanner(
    error: String,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    val infiniteTransition = rememberInfiniteTransition(label = "glow")
    val glowAlpha by infiniteTransition.animateFloat(
        initialValue = 0.3f,
        targetValue = 0.5f,
        animationSpec = infiniteRepeatable(
            animation = tween(1000, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "glow"
    )

    Box(
        modifier = modifier
            .padding(top = 80.dp)
            .fillMaxWidth()
            .padding(horizontal = 20.dp)
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(16.dp))
                .background(
                    Brush.horizontalGradient(
                        colors = listOf(
                            Color(0xFFFF6B6B).copy(alpha = glowAlpha),
                            Color(0xFFFF6B6B).copy(alpha = 0.15f)
                        )
                    )
                )
                .border(
                    BorderStroke(1.dp, Color(0xFFFF6B6B).copy(alpha = 0.3f)),
                    RoundedCornerShape(16.dp)
                )
                .clickable { onDismiss() }
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(Color(0xFFFF6B6B))
                )
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    color = Color.White.copy(alpha = 0.95f),
                    modifier = Modifier.weight(1f)
                )
                Text(
                    text = "×",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White.copy(alpha = 0.7f)
                )
            }
        }
    }
}
