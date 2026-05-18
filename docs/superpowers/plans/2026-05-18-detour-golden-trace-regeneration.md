# Detour Golden Trace Regeneration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure `DetourScenarioGoldenTest` rewrites `test_data/ty225_short_detour_android_trace.jsonl` on every run of the targeted unit test.

**Architecture:** Keep the change inside the existing golden test. The test should create or truncate the trace file before processing the scenario, then close the pipeline so the writer flushes the finished JSONL. This keeps the fixture regeneration behavior deterministic without changing the production pipeline.

**Tech Stack:** Kotlin, Robolectric, Gradle unit tests

---

### Task 1: Regenerate the golden trace file during the test run

**Files:**
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/DetourScenarioGoldenTest.kt:76-236`

- [ ] **Step 1: Confirm the current test writes to a fixed fixture path**

```kotlin
traceFile = testDataFile(ANDROID_TRACE_FILENAME)
```

- [ ] **Step 2: Make the setup remove any previous trace file before the test runs**

```kotlin
@Before
fun setup() {
    traceFile = testDataFile(ANDROID_TRACE_FILENAME)
    if (traceFile.exists()) {
        traceFile.delete()
    }
    routeData = TestDataLoader.loadRouteData(SHORT_DETOUR)
}
```

- [ ] **Step 3: Keep the scenario run writing to the same fixed path**

```kotlin
val scenarioPipeline = if (scenario == SHORT_DETOUR) {
    pipeline?.close()
    DetectionPipeline().also {
        it.initialize(routeData, traceFile = scenarioTraceFile)
        pipeline = it
    }
} else {
    DetectionPipeline().also {
        it.initialize(TestDataLoader.loadRouteData(scenario), traceFile = scenarioTraceFile)
    }
}
```

- [ ] **Step 4: Run the targeted test and confirm the file is recreated**

Run:

```bash
cd android
rtk ./gradlew :app:testDebugUnitTest --tests com.busarrival.app.scenarios.DetourScenarioGoldenTest
```

Expected: the test passes and `test_data/ty225_short_detour_android_trace.jsonl` exists after the run with fresh contents.

