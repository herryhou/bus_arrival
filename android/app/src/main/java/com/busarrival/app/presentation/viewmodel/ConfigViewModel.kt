package com.busarrival.app.presentation.viewmodel

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteMetadata
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.domain.model.DetectionParameters
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class ConfigUiState(
    val routes: List<RouteMetadata> = emptyList(),
    val activeRouteId: String? = null,
    val parameters: DetectionParameters = DetectionParameters.defaults,
    val isLoading: Boolean = false,
    val error: String? = null
)

@HiltViewModel
class ConfigViewModel @Inject constructor(
    private val routeStorage: RouteStorageManager,
    private val preferences: DetectionPreferences
) : ViewModel() {

    private val _uiState = MutableStateFlow(ConfigUiState())
    val uiState: StateFlow<ConfigUiState> = _uiState.asStateFlow()

    init {
        loadRoutes()
    }

    fun loadRoutes() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true)

            val routes = routeStorage.loadAllMetadata()
            val activeRouteId = preferences.activeRouteUuid
            val parameters = preferences.getParameters()

            _uiState.value = ConfigUiState(
                routes = routes.sortedByDescending { it.timestamp },
                activeRouteId = activeRouteId,
                parameters = parameters,
                isLoading = false
            )
        }
    }

    fun addRoute(uri: Uri, name: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            val result = routeStorage.copyToInternal(uri, name)

            result.onSuccess { uuid ->
                setActiveRoute(uuid)
                loadRoutes()
            }.onFailure { e ->
                _uiState.value = _uiState.value.copy(
                    isLoading = false,
                    error = "Failed to add route: ${e.message}"
                )
            }
        }
    }

    fun deleteRoute(metadata: RouteMetadata) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true)

            val result = routeStorage.deleteRoute(metadata.uuid)

            result.onSuccess {
                if (_uiState.value.activeRouteId == metadata.uuid) {
                    preferences.activeRouteUuid = null
                }
                loadRoutes()
            }.onFailure { e ->
                _uiState.value = _uiState.value.copy(
                    isLoading = false,
                    error = "Failed to delete route: ${e.message}"
                )
            }
        }
    }

    fun setActiveRoute(uuid: String) {
        viewModelScope.launch {
            preferences.activeRouteUuid = uuid
            _uiState.value = _uiState.value.copy(activeRouteId = uuid)
        }
    }

    fun updateParameter(
        distanceWeight: Int? = null,
        speedWeight: Int? = null,
        progressErrorWeight: Int? = null,
        dwellTimeWeight: Int? = null,
        corridorSize: Int? = null
    ) {
        val currentParams = _uiState.value.parameters
        val newParams = DetectionParameters(
            distanceWeight = distanceWeight ?: currentParams.distanceWeight,
            speedWeight = speedWeight ?: currentParams.speedWeight,
            progressErrorWeight = progressErrorWeight ?: currentParams.progressErrorWeight,
            dwellTimeWeight = dwellTimeWeight ?: currentParams.dwellTimeWeight,
            corridorSize = corridorSize ?: currentParams.corridorSize
        )

        preferences.saveParameters(newParams)
        _uiState.value = _uiState.value.copy(parameters = newParams)
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }
}
