# Android App Handoff - Room Database Implementation

## Context

Pure Kotlin Android bus arrival detection app. Pipeline modules complete (map matching, Kalman, probability, FSM, recovery). Service layer done (foreground service + Location API). Next: Room database for event persistence.

## Current State

**Branch:** `feature/android-app`

**Completed (11/13 tasks):**
- ✅ Project structure (Compose + Room + Location deps)
- ✅ Semantic types (DistCm, SpeedCms, Prob8)
- ✅ Binary format parser (v5.1 sparse grid)
- ✅ Map matching (heading-constrained, spatial grid)
- ✅ Kalman filter (HDOP-adaptive)
- ✅ Dead reckoning (DR decay)
- ✅ Probability model (4-feature LUT)
- ✅ State machine (6-state FSM)
- ✅ Recovery (GPS jump)
- ✅ Foreground service (DetectionService + LocationManager)
- ✅ DetectionPipeline integration

**Remaining:**
- Room database layer
- Compose UI screens (map viz, history, config)
- Testing

## Next Task: Room Database

Create Room entities, DAOs, and database for arrival/departure persistence.

### Files to Create

```
app/src/main/java/com/busarrival/app/data/local/
├── db/AppDatabase.kt           # Room database
├── entity/
│   ├── ArrivalEntity.kt        # Arrival record
│   ├── DepartureEntity.kt      # Departure record
│   └── TraceEntity.kt          # Trace record (optional)
└── dao/
    ├── ArrivalDao.kt           # Arrival queries
    ├── DepartureDao.kt         # Departure queries
    └── DetectionDao.kt         # Combined queries
```

### Key Requirements

1. **ArrivalEntity**
   - timestamp (Long)
   - stopIndex (Int)
   - sCm (Int)
   - probability (Int)
   - routeId (String) - for multi-route support

2. **DepartureEntity**
   - timestamp (Long)
   - stopIndex (Int)
   - sCm (Int)
   - dwellTimeS (Int)
   - routeId (String)

3. **DAOs**
   - Insert arrival/departure
   - Query by time range
   - Query by route
   - Get recent N events
   - Count events per stop

4. **Repository**
   - `DetectionRepository.kt` - bridges database + pipeline
   - Flow-based queries for UI

### Reference Types

From `app/src/main/java/com/busarrival/app/domain/model/StateModels.kt`:
```kotlin
data class ArrivalEvent(
    val timestamp: Long,
    val stopIndex: Int,
    val sCm: DistCm,
    val probability: Prob8
)

data class DepartureEvent(
    val timestamp: Long,
    val stopIndex: Int,
    val sCm: DistCm,
    val dwellTimeS: Int
)
```

### Integration Points

1. **DetectionService** - insert events on arrival/departure
2. **HistoryViewModel** - query events for display
3. **HistoryScreen** - display event list

### Testing

- Unit tests for DAOs
- Instrumented tests for database
- Test repository queries

## Quick Start Commands

```bash
# Continue on feature/android-app branch
cd android
./gradlew build

# Run tests
./gradlew test
./gradlew connectedAndroidTest
```

## Related Files

- Plan: `docs/android-plan.md`
- Spec: `docs/SPEC.md`
- Types: `app/.../domain/model/StateModels.kt`
- Service: `app/.../service/DetectionService.kt`
