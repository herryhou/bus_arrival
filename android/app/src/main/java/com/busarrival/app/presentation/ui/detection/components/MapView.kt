package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.busarrival.app.domain.model.RouteData
import com.google.android.gms.maps.model.CameraPosition
import com.google.android.gms.maps.model.LatLng
import com.google.android.gms.maps.model.LatLngBounds
import com.google.maps.android.compose.CameraPositionState
import com.google.maps.android.compose.GoogleMap
import com.google.maps.android.compose.Marker
import com.google.maps.android.compose.MarkerState
import com.google.maps.android.compose.Polyline
import com.google.maps.android.compose.rememberCameraPositionState

@Composable
fun MapView(
    routeData: RouteData?,
    currentSCm: Int,
    isCameraFollowEnabled: Boolean,
    modifier: Modifier = Modifier
) {
    val cameraPositionState = rememberCameraPositionState()

    val latLngs = remember(routeData) {
        routeData?.toLatLngs() ?: emptyList()
    }

    val stopLatLngs = remember(routeData) {
        routeData?.stops?.map { stop ->
            routeData.toLatLng(stop.progressCm)
        } ?: emptyList()
    }

    var currentPosition by remember(currentSCm) {
        mutableStateOf(
            routeData?.toLatLng(currentSCm)
        )
    }

    LaunchedEffect(currentSCm, routeData) {
        currentPosition = routeData?.toLatLng(currentSCm)
        if (isCameraFollowEnabled && currentPosition != null) {
            cameraPositionState.move(
                CameraPosition.fromLatLngZoom(currentPosition!!, 17f)
            )
        }
    }

    LaunchedEffect(latLngs) {
        if (latLngs.isNotEmpty()) {
            val bounds = LatLngBounds.builder()
            latLngs.forEach { bounds.include(it) }
            cameraPositionState.move(
                CameraPosition.fromLatLngZoom(
                    bounds.build().center,
                    15f
                )
            )
        }
    }

    GoogleMap(
        cameraPositionState = cameraPositionState,
        modifier = modifier.fillMaxSize()
    ) {
        // Route polyline
        if (latLngs.isNotEmpty()) {
            Polyline(
                points = latLngs,
                color = androidx.compose.ui.graphics.Color.Blue,
                width = 4f
            )
        }

        // Stop markers
        stopLatLngs.forEachIndexed { index, latLng ->
            Marker(
                state = MarkerState(position = latLng),
                title = "Stop $index",
                snippet = "Bus stop #$index"
            )
        }

        // Current position marker
        currentPosition?.let { pos ->
            Marker(
                state = MarkerState(position = pos),
                title = "Current Position",
                snippet = "sCm: $currentSCm"
            )
        }
    }
}

/**
 * Convert RouteData to list of LatLng for rendering.
 */
fun RouteData.toLatLngs(): List<LatLng> {
    val origin = LatLng(originLat / 1e6, originLon / 1e6)
    return nodes.map { node ->
        LatLng(
            origin.latitude + (node.yCm / 100.0) / 111111.0,
            origin.longitude + (node.xCm / 100.0) / (111111.0 * Math.cos(Math.toRadians(origin.latitude)))
        )
    }
}

/**
 * Convert progress along route (cm) to LatLng.
 */
fun RouteData.toLatLng(sCm: Int): LatLng? {
    val origin = LatLng(originLat / 1e6, originLon / 1e6)

    // Find node at or before this progress
    val nodeIndex = nodes.indexOfLast { it.cumDistCm <= sCm }
    if (nodeIndex == -1) return null

    val node = nodes[nodeIndex]
    val baseLatLng = LatLng(
        origin.latitude + (node.yCm / 100.0) / 111111.0,
        origin.longitude + (node.xCm / 100.0) / (111111.0 * Math.cos(Math.toRadians(origin.latitude)))
    )

    // Interpolate if not exactly at node
    val offsetCm = sCm - node.cumDistCm
    if (offsetCm > 0 && nodeIndex < nodes.size - 1) {
        val nextNode = nodes[nodeIndex + 1]
        val ratio = offsetCm.toFloat() / node.segLenMm.coerceAtLeast(1) / 10f

        return LatLng(
            baseLatLng.latitude + (nextNode.yCm - node.yCm) / 100.0 / 111111.0 * ratio,
            baseLatLng.longitude + (nextNode.xCm - node.xCm) / 100.0 / (111111.0 * Math.cos(Math.toRadians(baseLatLng.latitude))) * ratio
        )
    }

    return baseLatLng
}
