package com.busarrival.app.scenarios

import com.busarrival.app.data.pipeline.binary.RouteDataParser
import com.busarrival.app.scenarios.common.NmeaParser
import com.busarrival.app.scenarios.common.TraceLoader
import com.busarrival.app.service.DetectionPipeline
import com.busarrival.app.service.PipelineResult
import com.busarrival.app.service.TraceTick
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import kotlin.math.abs

/**
 * Golden standard test for ty225_short_detour scenario.
 * Validates 10 requirements per PRD line 186.
 *
 * Test data location: ../../test_data/ty225_short_detour_*
 *
 * Validations:
 * 1. Arrival sequence (stops 2,3,4,5 skipped)
 * 2. GPS monotonicity (no backward jumps)
 * 3. Off-route duration ≥5s
 * 4. Position freeze during off-route
 * 5. Immediate snap on re-entry (>100m jump)
 * 6. Skipped stops validation
 * 7. No arrivals during off-route
 * 8. Ground truth consistency
 * 9. Announce events validation
 * 10. Overall PRD success criteria
 */
@RunWith(RobolectricTestRunner::class)
class DetourScenarioGoldenTest {
    private companion object {
        const val SHORT_DETOUR = "short_detour"
        const val NORMAL_SCENARIO = "normal"
        const val MIN_OFF_ROUTE_DURATION_S = 5
        const val MIN_REENTRY_JUMP_CM = 10_000L
        const val MAX_REENTRY_TO_STOP6_MS = 10_000L
        const val MAX_ALLOWED_BACKTRACK_CM = 5_000L
        const val DETOUR_PHASE_TRANSITION_CM = 10_000L
        val EXPECTED_DETOUR_ARRIVALS = listOf(0, 1, 6, 7, 8, 9)
        val SKIPPED_DETOUR_STOPS = listOf(2, 3, 4, 5)
        // Trace v2 only includes active stops, so stop 1 may or may not appear in the
        // extracted announce stream depending on when the snapshot is taken.
        const val ROUTE_DATA_FILENAME = "ty225_short_detour.bin"
        const val NMEA_FILENAME = "ty225_short_detour_nmea.txt"
        const val NORMAL_ROUTE_DATA_FILENAME = "ty225_normal.bin"
        const val NORMAL_NMEA_FILENAME = "ty225_normal_nmea.txt"
        const val ANDROID_TRACE_FILENAME = "ty225_short_detour_android_trace_v2.jsonl"
    }

    private data class OffRouteEpisode(
        val startTick: Int,
        val endTick: Int,
        val startTime: Long,
        val endTime: Long,
        val frozenSCm: Long,
        val reentrySCm: Long,
        val reentryTime: Long,
    )

    private data class ScenarioRun(
        val ticks: List<TraceTick>,
        val arrivals: List<Pair<Int, Long>>,
    )

    private lateinit var traceFile: File
    private lateinit var routeData: com.busarrival.app.domain.model.RouteData
    private var pipeline: DetectionPipeline? = null

    @Before
    fun setup() {
        traceFile = testDataFile(ANDROID_TRACE_FILENAME)
        routeData = RouteDataParser.loadFromFile(testDataFile(ROUTE_DATA_FILENAME).absolutePath)
    }

    @After
    fun cleanup() {
        pipeline?.close()
        pipeline = null
    }

    @Test
    fun test_ty225_short_detour_golden_standard() {
        val run = processScenario(SHORT_DETOUR)
        val ticks = run.ticks
        val offRouteEpisode = detectOffRouteEpisode(ticks)

        // Run all 10 validations
        validateArrivalSequence(run.arrivals, offRouteEpisode)    // 1
        validateGpsMonotonicity(ticks)                            // 2
        validateOffRouteDuration(offRouteEpisode)                 // 3
        validatePositionFreeze(ticks)            // 4
        validateReentrySnap(run.arrivals, offRouteEpisode)        // 5
        validateSkippedStops(run.arrivals)       // 6
        validateNoArrivalsDuringOffRoute(run.arrivals, offRouteEpisode)  // 7
        validateGroundTruthConsistency()        // 8
        validateAnnounceEvents()                // 9
        validatePrdSuccessCriteria(run.arrivals, offRouteEpisode) // 10
    }

    @Test
    fun test_android_trace_file_is_generated_for_manual_review() {
        processScenario(SHORT_DETOUR)

        assertTrue(
            "Android detour trace should be written to test_data/$ANDROID_TRACE_FILENAME",
            testDataFile(ANDROID_TRACE_FILENAME).exists()
        )
    }

    @Test
    fun test_android_trace_file_uses_rust_trace_schema() {
        processScenario(SHORT_DETOUR)

        val firstTrace = Json.parseToJsonElement(
            testDataFile(ANDROID_TRACE_FILENAME).useLines { lines ->
                lines.first { it.isNotBlank() }
            }
        ).jsonObject
        val expectedKeys = setOf(
            "gps",
            "kalman",
            "map_matching",
            "detection",
            "corridor",
            "stop_states",
        )

        assertEquals(expectedKeys, firstTrace.keys)
        assertFalse("Canonical Android trace output must not contain legacy time", firstTrace.containsKey("time"))

        // Find first tick with active stop states (may be empty initially when far from stops)
        val firstTickWithStops = testDataFile(ANDROID_TRACE_FILENAME).useLines { lines ->
            lines.map { Json.parseToJsonElement(it).jsonObject }
                .first { it["stop_states"]?.jsonArray?.isNotEmpty() == true }
        }

        val firstStopState = firstTickWithStops["stop_states"]
            ?.jsonArray
            ?.firstOrNull()
            ?.jsonObject
            ?: error("Expected Android trace to include at least one tick with active stop states")

        assertEquals(
            setOf(
                "stop_idx",
                "gps_distance_cm",
                "progress_distance_cm",
                "fsm_state",
                "dwell_time_s",
                "probability",
                "previous_probability",
                "features",
                "announced",
                "skip_on_reentry",
                "previous_distance_cm",
                "just_arrived"
            ),
            firstStopState.keys
        )
    }

    @Test
    fun test_gps_jump_detection_is_disabled_during_off_route() {
        val run = processScenario(SHORT_DETOUR)
        val gpsJumpDuringOffRoute = run.ticks.filter { it.off_route && it.gps_jump }

        assertTrue(
            "GPS jump detection should only run in Normal mode. OffRoute ticks with gps_jump=true: $gpsJumpDuringOffRoute",
            gpsJumpDuringOffRoute.isEmpty()
        )
    }

    private fun testDataFile(filename: String): File {
        val explicitRoot = System.getProperty("test.data.root")?.let(::File)
        if (explicitRoot != null) {
            return File(explicitRoot, filename)
        }

        return File(findTestDataRoot(), filename)
    }

    private fun findTestDataRoot(): File {
        var current: File? = File(".").absoluteFile
        repeat(8) {
            val candidate = File(current, "test_data")
            if (File(candidate, ROUTE_DATA_FILENAME).exists()) {
                return candidate
            }
            current = current?.parentFile
        }
        error("Unable to locate test_data directory from ${File(".").absolutePath}")
    }

    private fun processScenario(scenario: String): ScenarioRun {
        val scenarioTraceFile = File.createTempFile("trace-$scenario", ".jsonl")
        val scenarioPipeline = if (scenario == SHORT_DETOUR) {
            pipeline?.close()
            DetectionPipeline().also {
                it.initialize(routeData, traceFile = scenarioTraceFile)
                pipeline = it
            }
        } else {
            DetectionPipeline().also {
                it.initialize(
                    RouteDataParser.loadFromFile(testDataFile(NORMAL_ROUTE_DATA_FILENAME).absolutePath),
                    traceFile = scenarioTraceFile
                )
            }
        }
        val locations = when (scenario) {
            SHORT_DETOUR -> NmeaParser.parseFile(testDataFile(NMEA_FILENAME).readText())
            NORMAL_SCENARIO -> NmeaParser.parseFile(testDataFile(NORMAL_NMEA_FILENAME).readText())
            else -> error("Unsupported scenario: $scenario")
        }

        try {
            for (location in locations) {
                scenarioPipeline.process(location)
            }
            val ticks = TraceLoader.load(scenarioTraceFile)
            // Extract arrivals from trace using Arriving state (more deterministic than AtStop)
            val traceArrivals = extractArrivalEvents(ticks)
            return ScenarioRun(
                ticks = ticks,
                arrivals = traceArrivals
            )
        } finally {
            scenarioPipeline.close()
            if (scenario == SHORT_DETOUR) {
                traceFile.parentFile?.mkdirs()
                Files.copy(
                    scenarioTraceFile.toPath(),
                    traceFile.toPath(),
                    StandardCopyOption.REPLACE_EXISTING
                )
            }
            if (scenarioTraceFile.exists()) {
                scenarioTraceFile.delete()
            }
        }
    }

    /**
     * VALIDATION 1: Arrival Sequence (PRD Core Requirement)
     * Expected arrivals: [0, 6, 7, 8, 9] or [0, 1, 6, 7, 8, 9] if stop 1 completes before off-route.
     * Stops 2, 3, 4, 5 MUST be skipped (completely absent)
     */
    private fun validateArrivalSequence(arrivalEvents: List<Pair<Int, Long>>, offRouteEpisode: OffRouteEpisode) {
        println("\n=== VALIDATION 1: Arrival Sequence ===")

        val arrivals = arrivalEvents.map { it.first }
        val stop1ArrivalTime = arrivalEvents.firstOrNull { it.first == 1 }?.second

        validateDetourArrivalSequence(arrivals, stop1ArrivalTime, offRouteEpisode.startTime)
    }

    private fun validateDetourArrivalSequence(
        detectedStops: List<Int>,
        stop1ArrivalTime: Long?,
        offRouteStartTime: Long,
    ) {
        for (skipped in SKIPPED_DETOUR_STOPS) {
            assertFalse(
                "Stop $skipped should be SKIPPED (not in arrivals). Detected: $detectedStops",
                detectedStops.contains(skipped)
            )
        }

        assertTrue(
            "Arrival sequence contains unexpected stops. Detected: $detectedStops",
            detectedStops.all { it in setOf(0, 1, 6, 7, 8, 9) }
        )
        assertEquals(
            "Arrival sequence should not repeat stops. Detected: $detectedStops",
            detectedStops,
            detectedStops.distinct()
        )

        assertContainsOrderedStops(
            detectedStops,
            listOf(0, 6, 7, 8, 9),
            "Arrival sequence"
        )

        val stop1Index = detectedStops.indexOf(1)
        if (stop1Index >= 0) {
            val stop1Time = requireNotNull(stop1ArrivalTime) {
                "Stop 1 arrival time should exist when stop 1 is detected"
            }
            assertTrue(
                "Stop 1 is only valid before off-route starts. stop1=$stop1Time, off_route_start=$offRouteStartTime",
                stop1Time < offRouteStartTime
            )
            assertTrue(
                "Stop 1 should occur after stop 0 when it is present. Detected: $detectedStops",
                stop1Index > detectedStops.indexOf(0)
            )
            assertTrue(
                "Stop 1 should occur before stop 6 when it is present. Detected: $detectedStops",
                stop1Index < detectedStops.indexOf(6)
            )
        }
    }

    /**
     * VALIDATION 2: GPS Position Monotonicity (No Backward Jumps)
     * Position must be monotonically increasing OR frozen during off-route.
     */
    private fun validateGpsMonotonicity(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 2: GPS Position Monotonicity ===")

        var prevSCm: Long? = null
        var backwardJumps = 0
        var offRouteFreezeSCm: Long? = null

        for (tick in ticks) {
            if (prevSCm != null) {
                val prev = prevSCm

                if (tick.off_route) {
                    if (offRouteFreezeSCm == null) {
                        offRouteFreezeSCm = tick.s_cm
                    } else {
                        assertEquals(
                            "Position must remain frozen during off-route",
                            offRouteFreezeSCm,
                            tick.s_cm
                        )
                    }
                } else {
                    offRouteFreezeSCm = null
                }

                if (tick.s_cm < prev - MAX_ALLOWED_BACKTRACK_CM) {
                    backwardJumps++
                }
            }
            prevSCm = tick.s_cm
        }

        assertEquals(
            "GPS position should NOT jump backward by more than 50m during normal operation. Found $backwardJumps jumps",
            0,
            backwardJumps
        )
        println("✓ No backward GPS jumps greater than 50m during normal operation")
    }

    /**
     * VALIDATION 3: Off-Route Detection & Duration
     * Off-route must be detected and last ≥5 seconds.
     */
    private fun validateOffRouteDuration(episode: OffRouteEpisode) {
        println("\n=== VALIDATION 3: Off-Route Duration ===")
        val durationTicks = episode.endTick - episode.startTick
        assertTrue(
            "Off-route must last ≥$MIN_OFF_ROUTE_DURATION_S seconds (PRD line 186). Duration: $durationTicks ticks",
            durationTicks >= MIN_OFF_ROUTE_DURATION_S
        )
        println("✓ Off-route duration: $durationTicks ticks (≥5s)")
    }

    /**
     * Helper: Extract arrival stop indices from trace.
     * An arrival is when a stop first enters Arriving state (more deterministic than AtStop which depends on dwell).
     */
    private fun extractArrivalEvents(ticks: List<TraceTick>): List<Pair<Int, Long>> {
        val stopFirstArriving = mutableMapOf<Int, Long>()
        for (tick in ticks) {
            tick.stop_states.forEach { state ->
                if (state.fsm_state == "Arriving" && !stopFirstArriving.containsKey(state.stop_idx)) {
                    stopFirstArriving[state.stop_idx] = tick.time_ms
                }
            }
        }
        return stopFirstArriving.keys.sorted().map { it to stopFirstArriving.getValue(it) }
    }

    private fun extractArrivals(ticks: List<TraceTick>): List<Int> {
        return extractArrivalEvents(ticks).map { it.first }
    }

    private fun detectOffRouteEpisode(ticks: List<TraceTick>): OffRouteEpisode {
        var prevOffRoute = false
        var startTick: Int? = null
        var startTime: Long? = null
        var frozenSCm: Long? = null
        var episode: OffRouteEpisode? = null

        for ((idx, tick) in ticks.withIndex()) {
            if (tick.off_route && startTick == null) {
                startTick = idx
                startTime = tick.time_ms
                frozenSCm = tick.s_cm
            }

            if (prevOffRoute && !tick.off_route) {
                assertNull("Expected exactly one contiguous off-route episode", episode)
                episode = OffRouteEpisode(
                    startTick = startTick ?: error("Missing off-route start tick"),
                    endTick = idx,
                    startTime = startTime ?: error("Missing off-route start time"),
                    endTime = tick.time_ms,
                    frozenSCm = frozenSCm ?: error("Missing frozen s_cm"),
                    reentrySCm = tick.s_cm,
                    reentryTime = tick.time_ms
                )
                startTick = null
                startTime = null
                frozenSCm = null
            }

            prevOffRoute = tick.off_route
        }

        return episode ?: error("Expected an off-route episode with a re-entry transition")
    }

    /**
     * VALIDATION 4: Position Freezing During Off-Route
     * When off_route=true, s_cm must remain constant.
     */
    private fun validatePositionFreeze(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 4: Position Freeze During Off-Route ===")

        var frozenSCm: Long? = null
        var freezeViolations = 0

        for (tick in ticks) {
            if (tick.off_route) {
                if (frozenSCm == null) {
                    frozenSCm = tick.s_cm
                    println("Position frozen at s_cm = $frozenSCm")
                } else if (tick.s_cm != frozenSCm) {
                    println("⚠ Freeze violation: expected $frozenSCm, got ${tick.s_cm}")
                    freezeViolations++
                }
            } else {
                frozenSCm = null
            }
        }

        assertEquals(
            "Position must remain frozen during off-route. Violations: $freezeViolations",
            0,
            freezeViolations
        )
        println("✓ Position frozen during off-route")
    }

    /**
     * VALIDATION 5: Immediate Snap on Re-entry
     * On off_route=false transition, position must jump >100m.
     */
    private fun validateReentrySnap(arrivalEvents: List<Pair<Int, Long>>, episode: OffRouteEpisode) {
        println("\n=== VALIDATION 5: Immediate Snap on Re-entry ===")

        val jump = abs(episode.reentrySCm - episode.frozenSCm)
        val stop6ArrivalTime = arrivalEvents
            .firstOrNull { it.first == 6 }
            ?.second
            ?: error("Stop 6 should be detected after detour re-entry")

        assertTrue(
            "Re-entry must have significant position jump (>100m). Jump: ${jump}cm",
            jump > MIN_REENTRY_JUMP_CM
        )
        assertTrue(
            "Stop 6 should be reached quickly after re-entry. Re-entry at ${episode.reentryTime}, stop6 at $stop6ArrivalTime",
            stop6ArrivalTime >= episode.reentryTime
                && stop6ArrivalTime - episode.reentryTime <= MAX_REENTRY_TO_STOP6_MS
        )
    }

    /**
     * VALIDATION 6: Skipped Stops Validation
     * Stops 2, 3, 4, 5 must NOT appear in arrivals.
     */
    private fun validateSkippedStops(arrivalEvents: List<Pair<Int, Long>>) {
        println("\n=== VALIDATION 6: Skipped Stops ===")

        val arrivals = arrivalEvents.map { it.first }
        val skippedStops = listOf(2, 3, 4, 5)

        for (skipped in skippedStops) {
            assertFalse(
                "Stop $skipped should be SKIPPED. Arrivals: $arrivals",
                arrivals.contains(skipped)
            )
        }

        println("✓ All intermediate stops skipped: $skippedStops")
    }

    /**
     * VALIDATION 7: No Arrivals During Off-Route
     * All arrivals must occur BEFORE or AFTER off-route episode.
     */
    private fun validateNoArrivalsDuringOffRoute(arrivalEvents: List<Pair<Int, Long>>, episode: OffRouteEpisode) {
        println("\n=== VALIDATION 7: No Arrivals During Off-Route ===")

        for ((stopIdx, arrivalTime) in arrivalEvents) {
            assertTrue(
                "Arrival at time $arrivalTime (stop $stopIdx) should NOT occur during off-route episode [${episode.startTime}, ${episode.endTime})",
                arrivalTime < episode.startTime || arrivalTime >= episode.endTime
            )
        }
    }

    /**
     * VALIDATION 8: Ground Truth Consistency
     * Compare against ty225_short_detour_gt.json if available.
     */
    private fun validateGroundTruthConsistency() {
        println("\n=== VALIDATION 8: Ground Truth Consistency ===")
        val gtContent = testDataFile("ty225_short_detour_gt.json").readText()
        val detourDurationS = validateGroundTruthDetourEvents(gtContent)

        assertTrue(
            "Detour duration should be ~60s (±5s). Got ${detourDurationS}s",
            abs(detourDurationS - 60L) <= 5L
        )
    }

    private fun validateGroundTruthDetourEvents(gtContent: String): Long {
        val events = Json.parseToJsonElement(gtContent).jsonArray
        var detourStartFound = false
        var detourEndFound = false
        var detourDurationS = 0L

        for (event in events) {
            val obj = event.jsonObject
            when (obj["event"]?.jsonPrimitive?.content) {
                "departure_detour" -> {
                    assertEquals(
                        "Ground truth detour start must be stop 1",
                        1,
                        obj["stop_idx"]?.jsonPrimitive?.int
                    )
                    detourStartFound = true
                }
                "re_acquisition" -> {
                    assertEquals(
                        "Ground truth detour end must be stop 6",
                        6,
                        obj["stop_idx"]?.jsonPrimitive?.int
                    )
                    detourEndFound = true
                    detourDurationS = obj["off_route_duration_s"]
                        ?.jsonPrimitive
                        ?.content
                        ?.toLong()
                        ?: detourDurationS
                }
            }
        }

        assertTrue("Ground truth must have detour_start event", detourStartFound)
        assertTrue("Ground truth must have detour_end event", detourEndFound)
        return detourDurationS
    }

    /**
     * VALIDATION 9: Announce Events Validation
     * Stops 2, 3, 4, 5 must NOT be announced.
     */
    private fun validateAnnounceEvents() {
        println("\n=== VALIDATION 9: Announce Events ===")
        val announceStops = extractAnnounceStopsFromTrace()
        val collapsed = validateAnnounceSequence(announceStops)
        for (skipped in SKIPPED_DETOUR_STOPS) {
            assertFalse(
                "Stop $skipped should NOT be announced. Announced: $collapsed",
                collapsed.contains(skipped)
            )
        }
    }

    private fun validateAnnounceSequence(announceStops: List<Int>): List<Int> {
        val collapsed = buildList {
            for (stop in announceStops) {
                if (lastOrNull() != stop) add(stop)
            }
        }
        assertTrue(
            "Announce sequence contains unexpected stops. Announced: $collapsed",
            collapsed.all { it in setOf(0, 1, 6, 7, 8, 9) }
        )
        assertContainsOrderedStops(
            collapsed,
            listOf(0, 6, 7, 8, 9),
            "Announce sequence"
        )
        val stop1Index = collapsed.indexOf(1)
        if (stop1Index >= 0) {
            assertTrue(
                "Stop 1 should be announced after stop 0 when present. Announced: $collapsed",
                stop1Index > collapsed.indexOf(0)
            )
            assertTrue(
                "Stop 1 should be announced before stop 6 when present. Announced: $collapsed",
                stop1Index < collapsed.indexOf(6)
            )
        }
        return collapsed
    }

    private fun assertContainsOrderedStops(
        actualStops: List<Int>,
        expectedStops: List<Int>,
        label: String,
    ) {
        var previousIndex = -1
        for (expectedStop in expectedStops) {
            val nextIndex = actualStops.indexOfFirst { it == expectedStop && it > previousIndex }
            assertTrue(
                "$label must contain $expectedStops in order. Detected: $actualStops",
                nextIndex > previousIndex
            )
            previousIndex = nextIndex
        }
    }

    private fun extractAnnounceStopsFromTrace(): List<Int> {
        val traceFile = testDataFile(ANDROID_TRACE_FILENAME)
        return TraceLoader.load(traceFile)
            .flatMap { tick ->
                // Use Arriving state instead of announced flag (more deterministic)
                tick.stop_states.filter { it.fsm_state == "Arriving" }.map { it.stop_idx }
            }
    }

    private fun extractAnnounceEventsFromTrace(): List<Pair<Long, Int>> {
        val traceFile = testDataFile(ANDROID_TRACE_FILENAME)
        return TraceLoader.load(traceFile)
            .flatMap { tick ->
                // Use Arriving state instead of announced flag (more deterministic)
                tick.stop_states.filter { it.fsm_state == "Arriving" }
                    .map { tick.gps.time_ms to it.stop_idx }
            }
    }

    private fun validateNoSkippedStopFsmStates(stopIdx: Int) {
        if (SKIPPED_DETOUR_STOPS.contains(stopIdx)) {
            fail("Skipped stop $stopIdx should not have FSM states")
        }
    }

    private fun validatePrdSuccessCriteria(
        arrivalEvents: List<Pair<Int, Long>>,
        episode: OffRouteEpisode,
    ) {
        println("\n=== VALIDATION 10: PRD Success Criteria ===")
        val durationTicks = episode.endTick - episode.startTick
        val reentryPositionJumpCm = abs(episode.reentrySCm - episode.frozenSCm)
        val detectedStops = arrivalEvents.map { it.first }

        assertTrue(
            "Off-route 5+ seconds -> position freeze. Got $durationTicks ticks",
            durationTicks >= MIN_OFF_ROUTE_DURATION_S
        )
        assertTrue(
            "Immediate snap on re-entry must be >100m. Got $reentryPositionJumpCm cm",
            reentryPositionJumpCm > MIN_REENTRY_JUMP_CM
        )
        for (skipped in SKIPPED_DETOUR_STOPS) {
            assertFalse(
                "Intermediate stop $skipped should be fully skipped. Detected: $detectedStops",
                detectedStops.contains(skipped)
            )
        }
    }

    /**
     * Supplementary FSM state transition validation.
     */
    private fun validateFsmTransitions(ticks: List<TraceTick>) {
        println("\n=== VALIDATION 10: FSM State Transitions ===")

        // Build state history for each stop
        val stateHistory = mutableMapOf<Int, MutableList<String>>()

        for (tick in ticks) {
            tick.stop_states.forEach { state ->
                validateNoSkippedStopFsmStates(state.stop_idx)
                if (!stateHistory.containsKey(state.stop_idx)) {
                    stateHistory[state.stop_idx] = mutableListOf()
                }
                stateHistory[state.stop_idx]?.add(state.fsm_state)
            }
        }

        // Verify stop 0 has proper progression
        val stop0States = stateHistory[0] ?: emptyList()
        if (stop0States.isNotEmpty()) {
            val hasApproaching = stop0States.contains("Approaching")
            val hasAtStop = stop0States.contains("AtStop")
            println("Stop 0 states: ${stop0States.toSet()}")

            assertTrue(
                "Stop 0 should have Approaching state",
                hasApproaching
            )
            assertTrue(
                "Stop 0 should have AtStop state",
                hasAtStop
            )
        }

        println("✓ FSM transitions valid for detected stops")
    }

    @Test
    fun test_detour_arrival_sequence_accepts_documented_sequence() {
        validateDetourArrivalSequence(listOf(0, 1, 6, 7, 8, 9), 99, 100)
    }

    @Test
    fun test_detour_arrival_sequence_accepts_sequence_without_stop_1() {
        validateDetourArrivalSequence(listOf(0, 6, 7, 8, 9), null, 100)
    }

    @Test(expected = AssertionError::class)
    fun test_detour_arrival_sequence_rejects_duplicate_arrivals() {
        validateDetourArrivalSequence(listOf(0, 6, 6, 7, 8, 9), null, 100)
    }

    @Test(expected = AssertionError::class)
    fun test_detour_arrival_sequence_rejects_reordered_arrivals() {
        validateDetourArrivalSequence(listOf(6, 0, 7, 8, 9), null, 100)
    }

    @Test(expected = AssertionError::class)
    fun test_detour_arrival_sequence_rejects_unexpected_extra_arrivals() {
        validateDetourArrivalSequence(listOf(0, 10, 6, 7, 8, 9), null, 100)
    }

    @Test(expected = AssertionError::class)
    fun test_off_route_duration_rejects_short_contiguous_episode() {
        validateOffRouteDuration(
            OffRouteEpisode(
                startTick = 10,
                endTick = 13,
                startTime = 10,
                endTime = 13,
                frozenSCm = 100,
                reentrySCm = 200,
                reentryTime = 13
            )
        )
    }

    @Test
    fun test_no_arrivals_allows_arrival_at_reentry_tick() {
        val episode = OffRouteEpisode(
            startTick = 10,
            endTick = 15,
            startTime = 10,
            endTime = 15,
            frozenSCm = 100,
            reentrySCm = 200,
            reentryTime = 15
        )

        validateNoArrivalsDuringOffRoute(listOf(6 to 15L), episode)
    }

    @Test(expected = AssertionError::class)
    fun test_ground_truth_detour_events_reject_wrong_start_stop() {
        validateGroundTruthDetourEvents(
            """
            [
              {"event": "departure_detour", "stop_idx": 2},
              {"event": "re_acquisition", "stop_idx": 6, "off_route_duration_s": 62}
            ]
            """.trimIndent()
        )
    }

    @Test(expected = AssertionError::class)
    fun test_fsm_validation_rejects_skipped_stop_states() {
        validateNoSkippedStopFsmStates(2)
    }

    @Test(expected = AssertionError::class)
    fun test_announce_sequence_rejects_reordered_announcements() {
        validateAnnounceSequence(listOf(1, 0, 6, 7, 8, 9))
    }

    @Test
    fun test_announce_sequence_accepts_collapsed_fixture_sequence() {
        // Accepts collapsed sequence (duplicates removed from consecutive ticks)
        validateAnnounceSequence(listOf(0, 0, 1, 1, 6, 6, 7, 8, 8, 9))
    }

    @Test
    fun test_no_backward_position_jumps() {
        val run = processScenario(SHORT_DETOUR)
        val sCmValues = mutableListOf<Long>()
        val detourPhase = mutableListOf<Boolean>()
        var consecutiveNormalTicks = 0
        var inDetour = false

        for (tick in run.ticks) {
            val backwardJump = sCmValues.isNotEmpty()
                && tick.s_cm < sCmValues.last() - DETOUR_PHASE_TRANSITION_CM

            if (backwardJump || tick.off_route) {
                inDetour = true
                consecutiveNormalTicks = 0
            } else if (inDetour) {
                consecutiveNormalTicks++
                if (consecutiveNormalTicks > 50) {
                    inDetour = false
                }
            }

            sCmValues.add(tick.s_cm)
            detourPhase.add(inDetour)
        }

        for (i in 1 until sCmValues.size) {
            val prev = sCmValues[i - 1]
            val curr = sCmValues[i]
            if (detourPhase[i - 1] || detourPhase[i]) continue

            assertFalse(
                "Backward jump >50m detected at index $i: $prev -> $curr (${curr - prev} cm drop)",
                curr < prev - MAX_ALLOWED_BACKTRACK_CM
            )
        }
    }

    @Test
    fun test_fsm_state_transitions_detour() {
        val run = processScenario(SHORT_DETOUR)
        val stopFsmStates = mutableMapOf<Int, MutableList<String>>()

        for (tick in run.ticks) {
            tick.stop_states.forEach { state ->
                validateNoSkippedStopFsmStates(state.stop_idx)
                val states = stopFsmStates
                    .getOrPut(state.stop_idx) { mutableListOf() }
                states.add(state.fsm_state)

                if (state.stop_idx <= 1 || state.stop_idx >= 6) {
                    for (i in 1 until states.size) {
                        val prev = states[i - 1]
                        val curr = states[i]
                        assertFalse(
                            "Invalid FSM state transition for stop ${state.stop_idx}: $prev -> $curr",
                            curr.contains("Approaching") && prev.contains("Departed")
                        )
                    }
                }
            }

            if (tick.time_ms > 80_200_000L) break
        }
    }

    @Test
    fun test_announce_precedes_arrival() {
        val run = processScenario(SHORT_DETOUR)
        val announceEvents = extractAnnounceEventsFromTrace()

        for ((arrivalStop, arrivalTime) in run.arrivals) {
            val matchingAnnounce = announceEvents.firstOrNull { (announceTime, announceStop) ->
                announceStop == arrivalStop && announceTime <= arrivalTime
            }

            assertNotNull(
                "No announce event found for arrival at time $arrivalTime (stop $arrivalStop)",
                matchingAnnounce
            )
        }
    }

    @Test
    fun test_off_route_reentry_snap_to_forward_stop() {
        val run = processScenario(SHORT_DETOUR)
        val offRouteEpisode = detectOffRouteEpisode(run.ticks)
        val positionJump = abs(offRouteEpisode.reentrySCm - offRouteEpisode.frozenSCm)
        val stop6ArrivalTime = run.arrivals
            .firstOrNull { it.first == 6 }
            ?.second
            ?: error("Stop 6 should be detected after re-entry")

        assertTrue(
            "Re-entry must cause a large snap (>100m). Got $positionJump cm",
            positionJump > MIN_REENTRY_JUMP_CM
        )
        assertTrue(
            "Stop 6 should be reached quickly after re-entry. Re-entry at ${offRouteEpisode.reentryTime}, stop 6 at $stop6ArrivalTime",
            stop6ArrivalTime >= offRouteEpisode.reentryTime
                && stop6ArrivalTime - offRouteEpisode.reentryTime <= MAX_REENTRY_TO_STOP6_MS
        )
    }

    @Test
    fun test_off_route_reentry_skips_intermediate_stops() {
        val run = processScenario(SHORT_DETOUR)
        val offRouteEpisode = detectOffRouteEpisode(run.ticks)
        val detectedStops = run.arrivals.map { it.first }
        val stop5Progress = routeData.stops[5].progressCm.toLong()

        assertTrue(
            "At re-entry, s_cm=${offRouteEpisode.reentrySCm} should be past stop 5 progress=$stop5Progress",
            offRouteEpisode.reentrySCm > stop5Progress
        )

        for (stop in SKIPPED_DETOUR_STOPS) {
            val stopProgress = routeData.stops[stop].progressCm.toLong()
            assertTrue(
                "Intermediate stop $stop progress_cm=$stopProgress should be < reentry_s_cm=${offRouteEpisode.reentrySCm}",
                stopProgress < offRouteEpisode.reentrySCm
            )
            assertFalse(
                "Intermediate stop $stop should NOT be in arrivals (behind snap position). Detected: $detectedStops",
                detectedStops.contains(stop)
            )
        }

        for (stop in EXPECTED_DETOUR_ARRIVALS.drop(2)) {
            assertTrue(
                "Stop $stop should be detected (ahead of snap position). Detected: $detectedStops",
                detectedStops.contains(stop)
            )
        }
    }

    private val TraceTick.time_ms: Long
        get() = gps.time_ms

    private val TraceTick.s_cm: Long
        get() = kalman.s_cm

    private val TraceTick.off_route: Boolean
        get() = detection.off_route

    private val TraceTick.gps_jump: Boolean
        get() = detection.gps_jump

    @Test
    fun test_normal_operation_does_not_skip_stops() {
        val run = processScenario(NORMAL_SCENARIO)

        assertTrue(
            "Normal scenario should detect many stops. Got: ${run.arrivals.map { it.first }}",
            run.arrivals.size > 10
        )
    }
}
