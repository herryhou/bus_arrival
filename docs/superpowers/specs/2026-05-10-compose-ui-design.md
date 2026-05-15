# Compose UI Design Spec

**Date:** 2026-05-10
**Branch:** feature/android-app
**Status:** Design approved, pending implementation

## Overview

Implement Compose UI screens for Android bus arrival detection app. Three-screen bottom navigation architecture with integrated map visualization.

## User Flow

1. **ConfigScreen** (entry point) - Load route file, configure parameters
2. **DetectionScreen** - Start service, view map + real-time status
3. **HistoryScreen** - Review past arrivals/departures

## Architecture

### Screens

| Screen | Purpose | Key Features |
|--------|---------|--------------|
| ConfigScreen | Route management, parameters | Route list, file picker, parameter sliders |
| DetectionScreen | Map + status split view | Google Maps, service controls, event stream |
| HistoryScreen | Event log | Filter chips, LazyColumn, empty state |

### ViewModels

- `ConfigViewModel` - Route CRUD, parameter persistence (new)
- `DetectionViewModel` - Service binding, event stream (exists, extend)
- `HistoryViewModel` - Repository queries, filter state (new)

### Data Layer (New Components)

```
data/
├── storage/
│   └── RouteStorageManager.kt    # Copy .bin to internal, metadata CRUD
└── preferences/
    └── DetectionPreferences.kt   # SharedPreferences wrapper for params
```

## Component Specifications

### ConfigScreen

**Layout:**
- Route list (LazyColumn)
  - Each item: name, timestamp, active indicator
  - Click to select as active
  - Long-press → delete confirmation
- FAB: "Add Route" → file picker
- Parameter section (expandable):
  - Slider: Distance weight (0-100)
  - Slider: Speed weight (0-100)
  - Slider: Progress error weight (0-100)
  - Slider: Dwell time weight (0-100)
  - Slider: Corridor size (-80m to +40m range)
  - Button: "Apply Parameters"

**State:**
```kotlin
data class ConfigUiState(
    val routes: List<RouteMetadata> = emptyList(),
    val activeRouteId: String? = null,
    val parameters: DetectionParameters = DetectionParameters.defaults,
    val isLoading: Boolean = false,
    val error: String? = null
)
```

**Actions:**
- Add route: File picker → copy to `files/routes/{uuid}.bin` → parse → save metadata
- Delete route: Confirm dialog → delete file + metadata
- Select active: Update SharedPreferences
- Update parameters: Update SharedPreferences

### DetectionScreen

**Layout (vertical split):**
- Top 60%: Google Maps Compose
  - Route polyline (blue, 4px)
  - Stop markers (default marker)
  - Current position (custom marker, updates via StateFlow)
  - Camera control: follow mode toggle
- Bottom 40%: Status panel
  - Start/Stop service button (prominent)
  - Service status indicator (running/stopped)
  - Current position: sCm, speed, mode
  - Recent events: last 5 (scrollable)

**State:**
```kotlin
data class DetectionUiState(
    val isServiceRunning: Boolean = false,
    val currentPosition: PositionSnapshot? = null,
    val recentEvents: List<PipelineEvent> = emptyList(),
    val isCameraFollowEnabled: Boolean = true,
    val error: String? = null
)
```

**Map rendering:**
- Convert RouteData nodes to LatLng (using originLat/originLon)
- Draw polyline: `Polyline(points = latLngs)`
- Stop markers: `Marker(position = stopLatLng, title = "Stop $index")`
- Position marker: Update on `PipelineEvent.PositionUpdate`

### HistoryScreen

**Layout:**
- Filter row:
  - Time chips: [Today] [Week] [All]
  - Stop dropdown: "All stops" or specific index
- Content:
  - LazyColumn of events
  - Each item: timestamp, stop index, type (arrival/departure), details
  - Grouped by day (sticky headers)
- Empty state: Illustration + "No events recorded"

**State:**
```kotlin
data class HistoryUiState(
    val events: List<HistoryEventItem> = emptyList(),
    val timeFilter: TimeFilter = TimeFilter.Today,
    val stopFilter: Int? = null, // null = all stops
    val isLoading: Boolean = false
)

enum class TimeFilter { Today, Week, All }
```

**Query logic:**
- Today: `getArrivalsByTimeRange(startOfDay, now)`
- Week: `getArrivalsByTimeRange(startOfWeek, now)`
- All: `getRecentArrivals(limit=1000)`
- Stop filter: Apply post-query or use DAO method

### Bottom Navigation

**3 tabs:**
- Config (icon: settings)
- Detection (icon: map/radar)
- History (icon: history/list)

**Behavior:**
- Start destination: Config (if no active route) or Detection (if route loaded)
- Maintain backstack per tab
- Show badge on Detection when service running

## Data Flow

### Route Loading Flow

```
ConfigScreen.addRoute()
  → FilePicker intent
  → RouteStorageManager.copyToInternal(sourceUri)
     → Copy to files/routes/{uuid}.bin
     → Parse with RouteDataParser.loadFromFile()
     → Validate format
  → RouteStorageManager.saveMetadata(name, uuid, timestamp)
     → Append to files/routes/metadata.json
  → ConfigViewModel.setActiveRoute(uuid)
     → Update SharedPreferences
```

### Detection Flow

```
DetectionScreen.startService()
  → DetectionService.startService(context)
  → Service loads active route from RouteStorageManager
  → Service starts location updates
  → Service emits PipelineEvent stream
  → DetectionViewModel.events: StateFlow<List<PipelineEvent>>
  → UI observes via collectAsState()
     → Map updates position marker
     → Status panel shows events
```

### History Query Flow

```
HistoryScreen.filterChanged()
  → HistoryViewModel.applyFilter(timeFilter, stopFilter)
  → DetectionRepository.getArrivalsByTimeRange(...)
  → Returns Flow<List<ArrivalEntity>>
  → ViewModel maps to HistoryEventItem
  → UI renders LazyColumn
```

## Error Handling

| Scenario | UI Response |
|----------|-------------|
| Invalid route file | Toast: "Invalid route format" |
| Copy failure | Toast: "Failed to save route" |
| No active route | Disable service start button, show "Load a route first" |
| Location permission denied | Show "Grant permission" button → settings |
| Service crash | Show "Service stopped" with retry button |
| Empty history | Show illustration + "No events yet" |
| Query error | Show error message + retry button |

## Testing

### Unit Tests

**RouteStorageManagerTest:**
- `copyToInternal()` saves file correctly
- `loadMetadata()` parses JSON
- `deleteRoute()` removes file + metadata entry

**ConfigViewModelTest:**
- `loadRoutes()` populates state
- `addRoute()` handles invalid files
- `setActiveRoute()` updates preferences

**HistoryViewModelTest:**
- `applyFilter()` computes correct time ranges
- Filter combinations work correctly

### UI Tests

**ConfigScreenTest:**
- Click FAB → file picker launches
- Select route → becomes active
- Delete route → confirmation dialog shown
- Adjust slider → parameter updates

**DetectionScreenTest:**
- Click start → service starts
- Service running → status updates
- Toggle camera follow → map camera behavior changes

**HistoryScreenTest:**
- Click time chip → filter applies
- Select stop → dropdown updates
- Empty state shows when no data

## Implementation Notes

### Dependencies (already present)

```gradle
// Maps Compose
implementation("com.google.maps.android:maps-compose:4.3.0")

// Room
implementation("androidx.room:room-ktx:2.6.1")

// Navigation
implementation("androidx.navigation:navigation-compose:2.7.7")
```

### File Structure

```
app/src/main/java/com/busarrival/app/
├── data/
│   ├── storage/
│   │   └── RouteStorageManager.kt          # NEW
│   └── preferences/
│       └── DetectionPreferences.kt         # NEW
├── presentation/
│   ├── ui/
│   │   ├── config/
│   │   │   ├── ConfigScreen.kt             # UPDATE
│   │   │   └── components/
│   │   │       ├── RouteListItem.kt        # NEW
│   │   │       └── ParameterSlider.kt      # NEW
│   │   ├── detection/
│   │   │   ├── DetectionScreen.kt          # UPDATE
│   │   │   └── components/
│   │   │       ├── MapView.kt              # NEW
│   │   │       └── StatusPanel.kt          # NEW
│   │   └── history/
│   │       ├── HistoryScreen.kt            # UPDATE
│   │       └── components/
│   │           ├── EventItem.kt            # NEW
│   │           └── FilterChips.kt          # NEW
│   ├── viewmodel/
│   │   ├── ConfigViewModel.kt              # NEW
│   │   └── HistoryViewModel.kt             # NEW
│   └── navigation/
│       └── BusArrivalNavGraph.kt           # UPDATE (add bottom nav)
```

### Key Integrations

**DetectionService binding:**
```kotlin
// In DetectionViewModel
private val _serviceConnection = mutableStateOf<DetectionService?>(null)
val events: StateFlow<List<PipelineEvent>> = _serviceConnection.value?.events ?: emptyFlow()
```

**Route rendering:**
```kotlin
// Convert RouteData nodes to LatLng
fun RouteData.toLatLngs(): List<LatLng> {
    val origin = LatLng(originLat / 1e6, originLon / 1e6)
    return nodes.map { node ->
        LatLng(
            origin.latitude + (node.yCm / 100.0) / 111111.0,
            origin.longitude + (node.xCm / 100.0) / (111111.0 * cos(origin.latitude))
        )
    }
}
```

## Success Criteria

1. User can load and manage multiple route files
2. Detection service starts/stops reliably
3. Map shows route, stops, and real-time position
4. History displays events with working filters
5. Navigation between screens works smoothly
6. All UI tests pass

## Open Questions

None - design approved.
