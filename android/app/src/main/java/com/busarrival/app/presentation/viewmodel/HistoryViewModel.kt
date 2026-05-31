package com.busarrival.app.presentation.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import com.busarrival.app.data.gpslog.GpsLogMetadata
import com.busarrival.app.data.gpslog.GpsLogStorageManager
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.service.GpsLogArchive
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.runBlocking
import java.io.File

data class LogManagerItem(
    val filename: String,
    val reference: String,
    val modifiedAtMillis: Long,
    val sizeBytes: Long,
    val isActive: Boolean,
    val canSimulate: Boolean = !isActive,
    val isSelected: Boolean = false
)

data class LogManagerUiState(
    val isLoading: Boolean = false,
    val logs: List<LogManagerItem> = emptyList(),
    val selectedCount: Int = 0,
    val canDeleteSelected: Boolean = false,
    val error: String? = null
)

class HistoryViewModel(application: Application) : AndroidViewModel(application) {
    private val preferences = DetectionPreferences(application)
    private val _uiState = MutableStateFlow(LogManagerUiState(isLoading = true))
    val uiState: StateFlow<LogManagerUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        reloadLogs(preserveSelection = true)
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    fun toggleSelection(reference: String) {
        updateSelection { item ->
            if (item.reference == reference) item.copy(isSelected = !item.isSelected) else item
        }
    }

    fun selectAll() {
        updateSelection { item -> item.copy(isSelected = true) }
    }

    fun clearSelection() {
        updateSelection { item -> item.copy(isSelected = false) }
    }

    fun shareSelected(): File? {
        val selected = _uiState.value.logs.filter { it.isSelected }
        if (selected.isEmpty()) return null
        return GpsLogArchive.createZip(getApplication(), selected.map { it.toMetadata() })
    }

    fun deleteSelected() {
        val selected = _uiState.value.logs.filter { it.isSelected }
        if (selected.isEmpty()) return
        if (selected.any { it.isActive }) {
            _uiState.value =
                _uiState.value.copy(error = "Stop recording before deleting the active log.")
            return
        }

        _uiState.value = _uiState.value.copy(isLoading = true, error = null)
        val context = getApplication<Application>()
        runBlocking(Dispatchers.IO) {
            selected.forEach { GpsLogStorageManager.deleteLog(context, it.reference) }
        }
        reloadLogs(preserveSelection = false)
    }

    fun requestSimulation(reference: String): Boolean {
        val item = _uiState.value.logs.firstOrNull { it.reference == reference } ?: return false
        if (item.isActive) {
            _uiState.value =
                _uiState.value.copy(error = "Stop recording before simulating the active log.")
            return false
        }

        preferences.setPendingSimulationGpsLog(item.reference, item.filename)
        _uiState.value = _uiState.value.copy(error = null)
        return true
    }

    private fun reloadLogs(preserveSelection: Boolean) {
        val currentSelectedRefs =
            if (preserveSelection) {
                _uiState.value.logs.filter { it.isSelected }.mapTo(mutableSetOf()) { it.reference }
            } else {
                emptySet<String>()
        }

        _uiState.value = _uiState.value.copy(isLoading = true, error = null)
        val context = getApplication<Application>()
        val logs =
            try {
                runBlocking(Dispatchers.IO) { GpsLogStorageManager.listLogs(context) }
            } catch (error: Exception) {
                _uiState.value =
                    LogManagerUiState(
                        isLoading = false,
                        error = error.message ?: "Failed to load GPS logs."
                    )
                return
            }

        _uiState.value =
            LogManagerUiState(
                isLoading = false,
                logs = logs.map { it.toItem(currentSelectedRefs.contains(it.reference)) },
                selectedCount = logs.count { currentSelectedRefs.contains(it.reference) },
                canDeleteSelected = logs.any { currentSelectedRefs.contains(it.reference) && it.isActive }.not() &&
                    logs.any { currentSelectedRefs.contains(it.reference) }
            )
    }

    private fun updateSelection(update: (LogManagerItem) -> LogManagerItem) {
        val current = _uiState.value.logs.map(update)
        val selectedCount = current.count { it.isSelected }
        _uiState.value =
            _uiState.value.copy(
                logs = current,
                selectedCount = selectedCount,
                canDeleteSelected = selectedCount > 0 && current.none { it.isSelected && it.isActive },
                error = null
            )
    }

    private fun LogManagerItem.toMetadata(): GpsLogMetadata {
        return GpsLogMetadata(
            filename = filename,
            reference = reference,
            modifiedAtMillis = modifiedAtMillis,
            sizeBytes = sizeBytes,
            isActive = isActive
        )
    }

    private fun GpsLogMetadata.toItem(isSelected: Boolean): LogManagerItem {
        return LogManagerItem(
            filename = filename,
            reference = reference,
            modifiedAtMillis = modifiedAtMillis,
            sizeBytes = sizeBytes,
            isActive = isActive,
            canSimulate = !isActive,
            isSelected = isSelected
        )
    }
}
