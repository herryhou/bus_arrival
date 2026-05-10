# Plan: Pure Kotlin Android Bus Arrival Detection App

## Context

Create a production-ready Android app for offline bus arrival detection using pure Kotlin. The app will process Android Location API data (not NMEA) through the full 3-phase pipeline: map matching → Kalman filtering → Bayesian arrival detection. Target: 3+ months development timeline with real-time detection, route visualization, historical data, and configuration features.

## Requirements Summary

| Aspect | Requirement |
|--------|-------------|
| **Input** | Android Location API (FusedLocationProviderClient) |
| **Processing** | Full 3-phase pipeline (map matching, Kalman, probability model, FSM) |
| **Output** | Real-time arrival/departure events |
| **Features** | Real-time detection, route visualization, historical data, configuration |
| **Platform** | Pure Kotlin (no JNI, no Rust) |
| **Timeline** | Production ready (3+ months) |

## Architecture

### Module Structure

```
app/                          # Main Android app
├── presentation/
│   ├── ui/                   # Compose UI screens
│   ├── viewmodel/            # ViewModels
│   └── navigation/           # Navigation graph
├── domain/                   # Business logic
│   ├── model/                # Domain models
│   ├── usecase/              # Use cases
│   └── repository/           # Repository interfaces
├── data/                     # Data layer
│   ├── repository/           # Repository implementations
│   ├── local/                # Room database
│   ├── assets/               # Route data files
│   └── pipeline/             # Detection pipeline (ported from Rust)
└── service/                  # Foreground service for GPS processing
```

### Core Pipeline Modules (Port from Rust)

```
data/pipeline/
├── types/                    # Semantic types (DistCm, SpeedCms, etc.)
├── binary/                   # Route data binary parser
├── localization/             # Map matching + Kalman filter
│   ├── mapmatcher/           # Heading-constrained map matching
│   ├── kalman/               # 1D Kalman filter
│   └── deadreckoning/        # GPS outage handling
├── detection/                # Arrival detection
│   ├── corridor/             # Stop corridor filtering
│   ├── probability/          # 4-feature probability model
│   ├── statemachine/         # FSM (Approaching → AtStop → Departed)
│   └── recovery/             # Stop index recovery
└── state/                    # Pipeline state management
```

## Implementation Phases

### Phase 1: Foundation (Weeks 1-3)

**Goal**: Core types, binary format parser, basic pipeline structure

1. **Semantic Type System**
   - Port `DistCm`, `SpeedCms`, `HeadCdeg`, `Prob8`, `Dist2` as Kotlin type aliases
   - `@JvmInline value class` for zero-overhead wrappers
   - Extension functions for unit conversions

2. **Binary Format Parser**
   - Parse v5.1 sparse grid format
   - Zero-copy ByteBuffer parsing
   - CRC32 validation
   - Asset loading from APK

3. **Domain Models**
   - `GpsPoint` (from Android Location)
   - `RouteNode`, `Stop`, `RouteData`
   - `KalmanState`, `DrState`
   - `StopState`, `FsmState`
   - `PositionSignals`, `ArrivalEvent`, `DepartureEvent`

### Phase 2: GPS Localization (Weeks 4-6)

**Goal**: Map matching and Kalman filter

1. **Map Matching** (Reference: `specs/01-map_matching.md`)
   - Heading-constrained segment search (±90° gate)
   - Spatial grid lookup (3×3 cell neighborhood)
   - Window search (last_idx ±2/+10)
   - Point-to-segment distance calculation
   - Heading difference calculation

2. **1D Kalman Filter** (Reference: `specs/02-kalman_filter.md`)
   - HDOP-adaptive gains (77/256, 51/256, 26/256, 13/256)
   - Prediction step: `s_pred = s_cm + v_cms`
   - Update step with fixed-point arithmetic
   - Velocity constraint: `v_cms ≥ 0`

3. **Dead Reckoning** (Reference: `specs/03-dead_reckoning.md`)
   - GPS outage detection
   - Position extrapolation: `s(t) = s(t-1) + v_filtered * dt`
   - EMA velocity filter: `v_filtered += 3*(v_gps - v_filtered)/10`
   - DR decay: `(9/10)^dt` normalization

4. **Off-Route Detection** (Reference: `specs/08-off_route_detection.md`)
   - Distance threshold: 50 m (OFF_ROUTE_D2_THRESHOLD)
   - Hysteresis: 5 ticks to confirm, 2 to clear
   - Position freezing behavior
   - Re-acquisition handling

### Phase 3: Arrival Detection (Weeks 7-9)

**Goal**: Probability model and state machine

1. **Stop Corridors** (Reference: `specs/04-stop_corridors.md`)
   - Dynamic corridor sizing (80m before, 40m after)
   - Approach corridor vs. at-stop corridor
   - Stop eligibility filtering

2. **Probability Model** (Reference: `specs/05-arrival_probability.md`)
   - 4-feature weighted model: F1 (distance), F2 (speed), F3 (progress), F4 (dwell)
   - Gaussian LUT: 256 entries for F1/F3
   - Logistic LUT: 128 entries for F2
   - Standard weights: (13, 6, 10, 3)
   - Adaptive weights for close stops: (14, 7, 11, 0)
   - Threshold: THETA_ARRIVAL = 191 (75%)

3. **State Machine** (Reference: `specs/06-state_machine.md`)
   - 6 states: Idle, Approaching, Arriving, AtStop, Departed, TripComplete
   - Per-stop FSM instances
   - One-time announcement rule
   - Dwell time tracking

4. **Recovery** (Reference: `specs/07-stop_recovery.md`)
   - GPS jump detection (>200 m)
   - Velocity-constrained search
   - Scoring: distance + index penalty + vel_penalty

### Phase 4: Android Integration (Weeks 10-12)

**Goal**: Location API, foreground service, persistence

1. **Location Service**
   - FusedLocationProviderClient integration
   - 1Hz location updates
   - Permission handling (FINE_LOCATION, BACKGROUND_LOCATION)
   - Location → GpsPoint conversion

2. **Foreground Service**
   - Pipeline execution in background
   - Notification with current state
   - Battery optimization handling
   - Service lifecycle management

3. **Room Database**
   - Arrival/departure event logging
   - Trace record storage
   - Route metadata caching
   - DAOs for queries

4. **Work Manager**
   - Periodic data sync (if needed)
   - Cleanup tasks
   - Backup/restore

### Phase 5: UI Development (Weeks 13-16)

**Goal**: Compose UI with MVVM architecture

1. **Real-Time Detection Screen**
   - Live map view with route overlay
   - Bus position marker
   - Active stops with probabilities
   - State machine visualization
   - Arrival/departure event log

2. **Route Visualization**
   - Google Maps or Mapbox integration
   - Route polyline rendering
   - Stop markers with corridor circles
   - Bus position with heading arrow
   - Camera tracking modes

3. **Historical Data Screen**
   - Event list (arrivals/departures)
   - Statistics dashboard
   - Trace replay
   - Export functionality

4. **Configuration Screen**
   - Route file loading
   - Threshold calibration
   - Debug toggle
   - Log level settings

### Phase 6: Testing & Polish (Weeks 17-20)

**Goal**: Production readiness

1. **Unit Tests**
   - Pipeline modules (map matching, Kalman, probability, FSM)
   - Repository implementations
   - Use case logic
   - Target: >80% coverage

2. **Integration Tests**
   - End-to-end pipeline processing
   - Database operations
   - Service lifecycle

3. **Instrumented Tests**
   - Location mocking
   - Pipeline with real GPS
   - UI flows

4. **Performance Testing**
   - Pipeline execution time (<10ms per update)
   - Memory profiling
   - Battery drain analysis

5. **Documentation**
   - API documentation
   - Architecture diagrams
   - User guide

6. **Distribution**
   - APK signing
   - Play Store listing
   - Release notes

## Key Technical Decisions

### Why Pure Kotlin (Not JNI)?

| Factor | Pure Kotlin | Rust + JNI |
|--------|-------------|------------|
| **Development time** | ~20 weeks | ~30 weeks |
| **Complexity** | Lower | Higher |
| **Debugging** | Easier | Harder (Rust stack traces in Kotlin) |
| **Performance** | ~3.2μs per update | ~3.2μs per update (JNI overhead) |
| **Maintenance** | Single language | Dual language |

**Conclusion**: For Android-only app, pure Kotlin is simpler and equally fast.

### Location API vs NMEA

Use **FusedLocationProviderClient** (Location API):
- Better battery management
- Automatic sensor fusion
- No need for raw NMEA parsing
- Simpler permission model

### Arithmetic Strategy

Unlike embedded (no FPU), Android can use floats. However:
- **Keep integer types for consistency** with spec
- **Use Float/Double for intermediate calculations** where appropriate
- **Maintain semantic type system** for safety

## File Structure (Key Files)

### New Files to Create

```
app/src/main/java/com/busarrival/
├── presentation/
│   ├── ui/
│   │   ├── detection/DetectionScreen.kt
│   │   ├── history/HistoryScreen.kt
│   │   ├── config/ConfigScreen.kt
│   │   └── map/MapViewModel.kt
│   └── viewmodel/
│       └── DetectionViewModel.kt
├── domain/
│   ├── model/
│   │   ├── GpsPoint.kt
│   │   ├── ArrivalEvent.kt
│   │   └── DepartureEvent.kt
│   └── usecase/
│       └── ProcessLocationUseCase.kt
├── data/
│   ├── pipeline/
│   │   ├── types/
│   │   │   └── SemanticTypes.kt
│   │   ├── binary/
│   │   │   └── RouteDataParser.kt
│   │   ├── localization/
│   │   │   ├── mapmatcher/MapMatcher.kt
│   │   │   ├── kalman/KalmanFilter.kt
│   │   │   └── deadreckoning/DeadReckoning.kt
│   │   ├── detection/
│   │   │   ├── corridor/CorridorFilter.kt
│   │   │   ├── probability/ProbabilityModel.kt
│   │   │   ├── statemachine/StateMachine.kt
│   │   │   └── recovery/Recovery.kt
│   │   └── state/
│   │       └── PipelineState.kt
│   ├── local/
│   │   ├── db/AppDatabase.kt
│   │   ├── entity/ArrivalEntity.kt
│   │   └── dao/ArrivalDao.kt
│   └── repository/
│       └── PipelineRepository.kt
└── service/
    └── DetectionService.kt
```

### Assets

```
app/src/main/assets/
└── routes/
    ├── ty225.bin          # Route data files
    └── other_route.bin
```

## Dependencies

```gradle
// Core
implementation("androidx.core:core-ktx:1.12.0")
implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")

// Compose
implementation(platform("androidx.compose:compose-bom:2024.02.00"))
implementation("androidx.compose.ui:ui")
implementation("androidx.compose.ui:ui-tooling-preview")
implementation("androidx.compose.material3:material3")
implementation("androidx.activity:activity-compose:1.8.2")

// ViewModel
implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.7.0")

// Location
implementation("com.google.android.gms:play-services-location:21.1.0")

// Maps
implementation("com.google.android.gms:play-services-maps:18.2.0")
implementation("com.google.maps.android:maps-compose:4.3.0")

// Room
implementation("androidx.room:room-runtime:2.6.1")
implementation("androidx.room:room-ktx:2.6.1")
kapt("androidx.room:room-compiler:2.6.1")

// Coroutines
implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")

// Work Manager
implementation("androidx.work:work-runtime-ktx:2.9.0")

// Testing
testImplementation("junit:junit:4.13.2")
testImplementation("io.mockk:mockk:1.13.8")
androidTestImplementation("androidx.test.ext:junit:1.1.5")
```

## Verification

### End-to-End Testing

1. **Load route data**: Verify binary format parsing
2. **Process location**: Map matching → Kalman → detection
3. **Emit events**: Arrival/departure events logged
4. **UI updates**: Real-time display of state
5. **Persistence**: Events saved to database
6. **Battery**: <5% per hour of active tracking

### Success Criteria

- [ ] Pipeline processes GPS updates at 1Hz without lag
- [ ] Arrival detection accuracy matches Rust implementation
- [ ] Map visualization shows correct route and bus position
- [ ] Historical data view shows past arrivals
- [ ] Configuration screen allows route loading and threshold adjustment
- [ ] Battery usage acceptable for full-day operation
- [ ] App passes all unit and integration tests
- [ ] Ready for Play Store distribution

## Reference Files (From Existing Codebase)

- `docs/specs/00-constraints.md` - Semantic type definitions
- `docs/specs/01-map_matching.md` - Map matching algorithm
- `docs/specs/02-kalman_filter.md` - Kalman filter equations
- `docs/specs/03-dead_reckoning.md` - Dead reckoning logic
- `docs/specs/04-stop_corridors.md` - Corridor filtering
- `docs/specs/05-arrival_probability.md` - Probability model
- `docs/specs/06-state_machine.md` - FSM transitions
- `docs/specs/07-stop_recovery.md` - Recovery algorithm
- `docs/specs/08-off_route_detection.md` - Off-route handling
- `crates/shared/src/lib.rs` - Core type definitions
- `crates/shared/src/probability_constants.rs` - Constants
- `crates/pipeline/src/lib.rs` - Pipeline structure
- `crates/pipeline/gps_processor/src/` - GPS processing reference
- `crates/pipeline/detection/src/` - Detection logic reference
