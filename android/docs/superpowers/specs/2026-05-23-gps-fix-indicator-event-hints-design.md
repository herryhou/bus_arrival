# GPS Fix Indicator & Event Hints Design

**Date:** 2026-05-23
**Status:** Approved
**Component:** Android App UI

## Overview

Add visual feedback for GPS acquisition and event notifications to the bus arrival detection app. Users see GPS fix status (progressive states + accuracy/satellites) and receive toast notifications for arrival/departure events.

## Problem Statement

Current UI shows GPS data (position, speed) but lacks:
1. **GPS fix status** - No indication of acquisition progress or signal quality
2. **Event feedback** - Arrivals/departures only appear in static list, no prominent notification

## Requirements

1. **GPS Fix Indicator**
   - Show progressive acquisition states (No signal → Searching → Acquiring → Ready)
   - Display accuracy (±Xm) and satellite count when available
   - Map overlay: heading arrow when GPS ready + valid bearing

2. **Event Hints**
   - Toast notifications for Approaching/Arriving/AtStop/Depart events
   - Auto-dismiss after 5 seconds
   - Color-coded by event type

3. **Placement**
   - GPS status: Mixed into Position/Speed row in StatusPanel
   - Heading arrow: MapView, centered on vehicle
   - Toasts: Overlay at top of DetectionScreen

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      DetectionViewModel                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ _gpsFixState │  │ _eventHints  │  │ _uiState     │      │
│  │   (Flow)     │  │   (Flow)     │  │   (Flow)     │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │              │
│  ┌──────▼─────────────────▼─────────────────▼───────┐      │
│  │            handleServiceEvent()                   │      │
│  │  - Updates GPS fix state                          │      │
│  │  - Emits EventHint for arrivals/departures        │      │
│  └──────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────┘
         │                   │                   │
         ▼                   ▼                   ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│   MapView    │   │ StatusPanel  │   │ ToastHost    │
│  (arrow)     │   │ (GPS row)    │   │ (hints)      │
└──────────────┘   └──────────────┘   └──────────────┘
```

## Data Models

### GpsFixState
```kotlin
sealed class GpsFixState {
    data object NoSignal : GpsFixState()
    data object Searching : GpsFixState()
    data class Acquiring(val satellites: Int) : GpsFixState()
    data class Ready(
        val accuracyM: Float,
        val satellites: Int,
        val bearing: Float?
    ) : GpsFixState()
}
```

**State transitions:**
- `NoSignal`: First location not received
- `Searching`: Receiving locations but accuracy > 20m
- `Acquiring`: accuracy ≤ 20m, satellites < 6
- `Ready`: accuracy ≤ 10m, satellites ≥ 6

### EventHint
```kotlin
data class EventHint(
    val type: HintType,
    val stopIndex: Int,
    val timestamp: Long = System.currentTimeMillis()
)

enum class HintType { APPROACHING, ARRIVING, ATSTOP, DEPART }
```

### Extended PipelineEvent.PositionUpdate
```kotlin
data class PositionUpdate(
    val sCm: Int,
    val vCms: Int,
    val mode: String = "Normal",
    val activeStopIndex: Int = -1,
    val activeStopState: String = "Idle",
    // NEW fields:
    val accuracyM: Float = Float.MAX_VALUE,
    val satellites: Int = 0,
    val bearing: Float? = null
) : PipelineEvent()
```

## Implementation Details

### DetectionViewModel Changes

**New state flows:**
```kotlin
private val _gpsFixState = MutableStateFlow<GpsFixState>(GpsFixState.NoSignal)
val gpsFixState: StateFlow<GpsFixState> = _gpsFixState.asStateFlow()

private val _eventHints = MutableStateFlow<EventHint?>(null)
val eventHints: StateFlow<EventHint?> = _eventHints.asStateFlow()
```

**handleServiceEvent() updates:**
- Compute `GpsFixState` from `PositionUpdate` metadata
- Emit `EventHint` on Arrival/Departure events
- Auto-dismiss hints after 5s (UI layer)

### DetectionService Changes

**GpsMetadata extraction:**
```kotlin
fun GpsMetadata(location: android.location.Location): GpsMetadata {
    return GpsMetadata(
        accuracyM = location.accuracy,
        satellites = location.extras?.getInt("satellites", 0) ?: 0,
        bearing = if (location.hasBearing()) location.bearing else null
    )
}
```

### UI Components

#### 1. VehicleHeadingMarker (MapView)
- Renders rotating arrow icon
- Only visible when `gpsFixState is Ready && bearing != null`
- 32dp size, primary color, rotates by bearing degrees

#### 2. GpsStatusRow (StatusPanel)
- Replaces current Position/Speed row
- 3 columns: GPS status, Position, Speed
- GPS column shows: state text + color (Green=Ready, Gray=other)

#### 3. EventToastHost (DetectionScreen)
- Top overlay, auto-dismisses after 5s
- EventToast per hint with icon + color:
  - Approaching: Blue, walking icon
  - Arriving: Orange, location icon
  - AtStop: Green, checkmark icon
  - Depart: Purple, exit icon

## Testing Strategy

### Unit Tests
- `computeGpsFixState()` with various accuracy/satellite combinations
- GPS state transitions (NoSignal → Searching → Acquiring → Ready)
- EventHint emission on Arrival/Departure events

### Integration Tests
- DetectionService emits PositionUpdate with GPS metadata
- ViewModel correctly updates all state flows
- Toast auto-dismiss timing

### UI Tests (Compose)
- MapView shows arrow only when Ready + bearing valid
- StatusPanel displays correct GPS text for each state
- ToastHost renders and dismisses hints

## Files to Modify

1. `DetectionViewModel.kt` - Add GPS fix state, event hints, compute logic
2. `DetectionService.kt` - Extend PositionUpdate, add GpsMetadata extraction
3. `LocationManager.kt` - Add GpsMetadata helper function
4. `StatusPanel.kt` - Replace Position/Speed row with GpsStatusRow
5. `MapView.kt` - Add VehicleHeadingMarker composable
6. `DetectionScreen.kt` - Add EventToastHost, wire up new state flows

## Success Criteria

1. GPS fix state visible in StatusPanel with progressive updates
2. Heading arrow appears on MapView when GPS ready
3. Toast notifications appear for arrivals/departures
4. All toasts auto-dismiss within 5 seconds
5. No regressions to existing functionality
