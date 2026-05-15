# Android Trace Output Design

**Date:** 2026-05-15
**Status:** Approved
**Goal:** Full parity with Rust golden test coverage (10 validations)

## Problem

Android `DetourScenarioTest.kt` has only basic assertions (4 test methods, 203 lines). Cannot validate:
- GPS position monotonicity (no `s_cm` history)
- Off-route duration ≥5s (no `off_route` flag)
- Position freezing during off-route
- Immediate snap on re-entry
- Ground truth consistency
- FSM state transitions

Rust `ty225_short_detour_golden.rs` has 10 comprehensive validations (979 lines) against `trace.jsonl`.

## Solution: Add Trace Output to Android Pipeline

### Architecture

```
DetectionPipeline
    ├── TraceWriter (optional, writes trace.jsonl)
    └── process() → PipelineResult + trace tick
```

**Key decision:** Trace output optional via parameter. When `traceFile = null`, `TraceWriter` is not instantiated (zero allocation, zero overhead).

### Data Structures

```kotlin
@Serializable
data class TraceTick(
    val time: Long,
    val s_cm: Long,
    val off_route: Boolean,
    val stop_states: List<StopStateEntry>?
)

@Serializable
data class StopStateEntry(
    val stop_idx: Int,
    val fsm_state: String,  // "Approaching", "Arriving", "AtStop", "Departed"
    val skip_on_reentry: Boolean = false
)
```

**Format:** JSONL (one JSON object per line), exact compatibility with Rust trace format. Uses `kotlinx.serialization` for JSON encoding.

### TraceWriter Component

```kotlin
class TraceWriter(
    private val file: File,
    private val enabled: Boolean = true
) {
    private val writer = if (enabled)
        BufferedWriter(FileWriter(file))
    else null

    fun write(tick: TraceTick) {
        if (enabled) {
            writer?.write(Json.encodeToString(tick))
            writer?.newLine()
        }
    }

    fun close() = writer?.close()
}
```

### DetectionPipeline Integration

```kotlin
class DetectionPipeline {
    private var traceWriter: TraceWriter? = null

    fun initialize(routeData: RouteData, traceFile: File? = null) {
        this.routeData = routeData
        // ...existing init...
        traceWriter = traceFile?.let { TraceWriter(it) }
    }

    fun process(location: Location): PipelineResult {
        // ...existing pipeline phases...

        // Capture state for trace (after all updates)
        traceWriter?.write(TraceTick(
            time = gps.timestamp,
            s_cm = signals.sCm,
            off_route = modeState.mode == Mode.OffRoute,
            stop_states = stopStates.map { (idx, state) ->
                StopStateEntry(idx, state.fsmState.name)
            }
        ))

        return result
    }
}
```

### Test Architecture

**New file:** `DetourScenarioGoldenTest.kt` (replaces basic `DetourScenarioTest.kt`)

```kotlin
@RunWith(RobolectricTestRunner::class)
class DetourScenarioGoldenTest {

    private lateinit var traceFile: File
    private lateinit var pipeline: DetectionPipeline

    @Before
    fun setup() {
        traceFile = File.createTempFile("trace", ".jsonl")
        pipeline = DetectionPipeline()
        pipeline.initialize(routeData, traceFile = traceFile)
    }

    @After
    fun cleanup() {
        traceFile.delete()
    }

    @Test
    fun test_ty225_short_detour_golden_standard() {
        // Process NMEA through pipeline
        for (location in locations) {
            pipeline.process(location)
        }

        // Load trace
        val ticks = TraceLoader.load(traceFile)

        // Run 10 validations (exact port from Rust)
        validateArrivalSequence(ticks)           // PRD core
        validateGpsMonotonicity(ticks)           // No backward jumps
        validateOffRouteDuration(ticks)          // ≥5s
        validatePositionFreeze(ticks)            // s_cm constant during off_route
        validateReentrySnap(ticks)               // >100m jump
        validateSkippedStops(ticks)              // Stops 2,3,4,5 not in arrivals
        validateNoArrivalsDuringOffRoute(ticks)  // Detection suppressed
        validateGroundTruthConsistency(ticks)    // vs ty225_short_detour_gt.json
        validateAnnounceEvents(ticks)            // Precede arrivals
        validateFsmTransitions(ticks)            // State progression
    }
}
```

**Helper:**
```kotlin
object TraceLoader {
    fun load(file: File): List<TraceTick> =
        file.readLines().map { Json.decodeFromString<TraceTick>(it) }
}
```

## 10 Validations (Port from Rust)

| # | Validation | PRD Requirement |
|---|------------|-----------------|
| 1 | Arrival sequence | Stops 2,3,4,5 skipped |
| 2 | GPS monotonicity | No backward jumps (except detour) |
| 3 | Off-route duration | ≥5 seconds per PRD line 186 |
| 4 | Position freeze | s_cm constant when off_route=true |
| 5 | Immediate snap | >100m jump on re-entry |
| 6 | Skipped stops | Stops 2,3,4,5 NOT in arrivals |
| 7 | No arrivals during off-route | Detection suppressed |
| 8 | Ground truth | Match ty225_short_detour_gt.json |
| 9 | Announce events | Precede arrivals |
| 10 | FSM transitions | Approaching → Arriving → AtStop → Departed |

## Implementation Plan

**Phase 1: Core infrastructure**
1. Add `TraceTick`, `StopStateEntry` data classes
2. Implement `TraceWriter`
3. Integrate into `DetectionPipeline.initialize()`

**Phase 2: Test validation**
4. Create `DetourScenarioGoldenTest.kt`
5. Implement `TraceLoader`
6. Port 10 validation functions from Rust

**Phase 3: Cleanup**
7. Deprecate `DetourScenarioTest.kt` (keep as reference)
8. Verify all 10 validations pass

## Files Changed

- `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt` (add trace output)
- `android/app/src/test/java/com/busarrival/app/scenarios/DetourScenarioGoldenTest.kt` (new)
- `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoader.kt` (new helper)
- `android/app/src/main/java/com/busarrival/app/service/TraceTick.kt` (new data class)
- `android/app/src/main/java/com/busarrival/app/service/TraceWriter.kt` (new component)

## Success Criteria

- All 10 validations pass on `ty225_short_detour` scenario
- Trace output format identical to Rust (tool compatibility)
- Zero overhead when `traceFile = null` (production)
- Test execution time < 5 seconds per scenario

## References

- Rust golden test: `crates/pipeline/tests/scenarios/ty225_short_detour_golden.rs`
- PRD line 186: "脫離路線 5 秒後位置凍結，重入時直接 snap 至前方站點，中間站點全數跳過"
- Tech spec: `bus_arrival_tech_report_v8.md`
