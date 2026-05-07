## Final Architecture for RP2350 Bus Arrival Detection

### Core Design Principles

1. **Each component is a deterministic state machine** – pure `update(input) -> Output`, no hidden side effects.
2. **No shared mutable state** – coordinator owns all component states.
3. **Explicit event routing** – coordinator call components, collects outputs, then dispatches.
4. **All allocations static** – `heapless` containers with fixed capacities.
5. **Testable on host** – each component has a `no_std` core + `std` test harness.

---

## Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              COORDINATOR (main loop)                         │
│  Owns: all component states, flash, UART, timer                              │
│                                                                              │
│  ┌──────────────┐   ┌─────────────┐   ┌────────────┐   ┌───────────────┐   │
│  │ UART Reader  │   │ NMEA Parser │   │ Map Matcher│   │ Kalman Filter │   │
│  │ (async)      │──▶│ (stateless) │──▶│ (stateful) │──▶│ (stateful)    │   │
│  └──────────────┘   └─────────────┘   └────────────┘   └───────────────┘   │
│                                                                     │       │
│                                                                     ▼       │
│  ┌──────────────┐   ┌─────────────────┐   ┌─────────────────────────────┐  │
│  │ Persistence  │   │ Mode Controller │   │ Stop Detector               │  │
│  │ (async task) │◀──│ (stateful)      │──▶│ (stateful)                  │  │
│  └──────────────┘   └─────────────────┘   └─────────────────────────────┘  │
│                                                     │                       │
│                                                     ▼                       │
│                                              Arrival Events                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Component Specifications

### 1. UART Reader (Async, in coordinator)
- **Role:** Read bytes from UART with timeout, feed to NMEA Parser.
- **No separate component** – implemented directly in `main.rs` using Embassy.

### 2. NMEA Parser (Stateless)
```rust
pub enum NmeaEvent {
    GpsFix(GpsPoint),
    Invalid,
}

pub fn parse_nmea(buffer: &[u8]) -> Option<GpsPoint>;
```
**State:** None – pure function, uses a tiny accumulator inside (re‑entrant? better to keep tiny state: `FixAccumulator`). Actually keep a small state struct:

```rust
pub struct NmeaParser {
    acc: FixAccumulator,
}

impl NmeaParser {
    pub fn feed(&mut self, sentence: &str) -> Option<GpsPoint>;
}
```
Size: ~200 bytes.

### 3. Map Matcher
```rust
pub struct MapMatcher {
    route: &'static RouteData,
    last_seg_idx: u16,
    // heading relax flag (set externally)
}

pub struct MapMatchInput {
    gps: GpsPoint,
    relax_heading: bool,   // from mode controller
    is_first_fix: bool,    // from warmup
}

pub struct MapMatchOutput {
    pub z_cm: i32,           // raw GPS projection
    pub divergence_d2: i64,  // squared distance to best segment
    pub segment_idx: u16,
}

impl MapMatcher {
    pub fn update(&mut self, input: MapMatchInput) -> MapMatchOutput;
}
```
Size: ~8 bytes.

### 4. Kalman Filter (with Dead Reckoning)
```rust
pub enum KalmanInput {
    GpsFix { z_cm: i32, v_cms: i32, hdop_x10: u16 },
    NoFix { dt_seconds: u16 },   // dt since last fix, max 10
}

pub struct KalmanOutput {
    pub s_cm: i32,        // filtered position
    pub v_cms: i32,       // filtered speed
    pub confidence: u8,   // derived from HDOP and divergence
}

pub struct KalmanFilter {
    s_cm: i32,
    v_cms: i32,
    last_fix_time: Option<u64>,   // for DR dt calculation
    filtered_v_ema: i32,          // for DR speed
}

impl KalmanFilter {
    pub fn update(&mut self, input: KalmanInput, timestamp: u64) -> KalmanOutput;
}
```
Size: ~24 bytes.

### 5. Mode Controller (Split into three sub‑components, but unified in one struct for simplicity)

Actually combine them into one struct but keep internal functions clean.

```rust
pub struct ModeController {
    // Mode state
    mode: Mode,                    // Normal, OffRoute, Recovering
    frozen_s_cm: Option<i32>,
    
    // Hysteresis counters
    off_route_suspect: u8,
    off_route_clear: u8,
    
    // Timers
    off_route_since: Option<u64>,
    recovering_since: Option<u64>,
    
    // Warmup
    estimation_ticks: u8,
    detection_ticks: u8,
    total_ticks: u8,
    first_fix_received: bool,
    
    // Last known stop (for recovery)
    last_stop_idx: u8,
    
    // Recovery flag (to avoid re-entrant recovery)
    recovery_in_progress: bool,
}

pub struct ModeControlInput {
    pub divergence_d2: i64,
    pub current_z: i32,       // raw GPS projection
    pub timestamp: u64,
    pub is_first_fix: bool,
    pub recovery_result: Option<usize>,   // from external recovery function
}

pub struct ModeControlOutput {
    pub mode: Mode,
    pub position_for_detection: i32,   // depends on mode
    pub detection_enabled: bool,
    pub relax_heading: bool,
    pub trigger_recovery: Option<RecoveryParams>,
    pub stop_changed: Option<u8>,
    pub freeze_position: Option<i32>,   // for logging/trace
}

impl ModeController {
    pub fn update(&mut self, input: ModeControlInput) -> ModeControlOutput;
}
```
Size: ~48 bytes.

**RecoveryParams** includes hint_idx, current_z, elapsed seconds.

### 6. Stop Detector (FSM + Probability)
```rust
pub struct StopDetector {
    stops: &'static [Stop],                // from route data
    stop_states: heapless::Vec<StopState, MAX_STOPS>,  // fixed capacity
    // No other state
}

pub struct StopDetectorInput {
    pub s_cm: i32,
    pub v_cms: i32,
    pub timestamp: u64,
}

pub enum StopDetectorEvent {
    Announce { stop_idx: u8, s_cm: i32, v_cms: i32 },
    Arrival { stop_idx: u8, s_cm: i32, v_cms: i32, probability: u8 },
    Departure { stop_idx: u8, s_cm: i32, v_cms: i32 },
}

impl StopDetector {
    pub fn update(&mut self, input: StopDetectorInput) -> heapless::Vec<StopDetectorEvent, 3>;
}
```
Size: ~2-5 KB depending on MAX_STOPS (256 stops → about 5KB after padding).

### 7. Recovery (Pure Function)
```rust
pub fn recover(input: RecoveryInput) -> Option<usize>;
```
No state.

### 8. Persistence Manager (Async Task)
```rust
pub struct PersistenceManager {
    flash: FlashPeripheral,
    pending_state: Option<PersistedState>,
    cooldown_ticks: u8,
}

impl PersistenceManager {
    pub fn request_save(&mut self, state: PersistedState);
    pub async fn run(&mut self);   // background task
}
```
State: ~16 bytes + flash handle.

---

## Coordinator Logic (Pseudo‑code)

```rust
#[embassy_executor::main]
async fn main() {
    let (uart, flash) = init_peripherals();
    let route = RouteData::load(include_bytes!("...")).unwrap();
    
    let mut nmea = NmeaParser::new();
    let mut map = MapMatcher::new(&route);
    let mut kalman = KalmanFilter::new();
    let mut mode = ModeController::new();
    let mut stop_detector = StopDetector::new(route.stops());
    let mut persistence = PersistenceManager::new(flash);
    
    let mut line_buf = UartLineBuffer::new();
    let mut last_timestamp = 0u64;
    
    loop {
        // Read one NMEA sentence (async)
        match read_nmea_sentence(&mut uart, &mut line_buf).await {
            Some(sentence) => {
                if let Some(gps) = nmea.feed(sentence) {
                    // New complete fix
                    process_gps_fix(&mut map, &mut kalman, &mut mode, 
                                    &mut stop_detector, &mut persistence, 
                                    gps, &route).await;
                }
            }
            None => continue,
        }
        
        // Also handle timeout ticks? GPS timestamp changes drive the loop.
        // No extra timer needed.
    }
}

async fn process_gps_fix(
    map: &mut MapMatcher,
    kalman: &mut KalmanFilter,
    mode: &mut ModeController,
    stop_detector: &mut StopDetector,
    persistence: &mut PersistenceManager,
    gps: GpsPoint,
    route: &RouteData,
) {
    // 1. Map matching
    let map_out = map.update(MapMatchInput {
        gps: gps.clone(),
        relax_heading: mode.should_relax_heading(),  // query from mode
        is_first_fix: !mode.estimation_ready(),
    });
    
    // 2. Mode controller (first pass – may trigger recovery)
    let mode_input = ModeControlInput {
        divergence_d2: map_out.divergence_d2,
        current_z: map_out.z_cm,
        timestamp: gps.timestamp,
        is_first_fix: !mode.estimation_ready(),
        recovery_result: None,
    };
    let mut mode_out = mode.update(mode_input);
    
    // 3. If mode_out.trigger_recovery is Some, call recover() and feed result back
    if let Some(params) = mode_out.trigger_recovery.take() {
        let stops_vec = route.stops_collect(); // or &[Stop]
        let recovered = recover(RecoveryInput {
            gps_s_cm: params.current_z,
            filtered_v_cms: kalman.v_cms(),
            dt_s: params.elapsed_seconds,
            stops: &stops_vec,
            hint_idx: params.hint_idx,
            frozen_s_cm: mode.frozen_position(),
            search_window: 10,
        });
        let mode_input2 = ModeControlInput {
            divergence_d2: map_out.divergence_d2,
            current_z: map_out.z_cm,
            timestamp: gps.timestamp,
            is_first_fix: false,
            recovery_result: recovered,
        };
        mode_out = mode.update(mode_input2);
    }
    
    // 4. Kalman filter uses raw GPS projection and mode's relax flag
    let kalman_input = if gps.has_fix {
        KalmanInput::GpsFix { z_cm: map_out.z_cm, v_cms: gps.speed_cms.unwrap_or(0), hdop_x10: gps.hdop_x10.unwrap_or(30) }
    } else {
        // dt from last fix (tracked internally)
        KalmanInput::NoFix { dt_seconds: gps.timestamp.saturating_sub(kalman.last_fix_time()) as u16 }
    };
    let kalman_out = kalman.update(kalman_input, gps.timestamp);
    
    // 5. Stop detector (only if mode_out.detection_enabled)
    let detection_events = if mode_out.detection_enabled {
        stop_detector.update(StopDetectorInput {
            s_cm: mode_out.position_for_detection,
            v_cms: kalman_out.v_cms,
            timestamp: gps.timestamp,
        })
    } else {
        heapless::Vec::new()
    };
    
    // 6. Emit events to UART
    for ev in detection_events {
        write_arrival_event(&mut uart, ev).await;
    }
    
    // 7. Persistence if stop changed and rate limit allows
    if let Some(new_stop) = mode_out.stop_changed {
        persistence.request_save(PersistedState::new(kalman_out.s_cm, new_stop));
    }
}
```

---

## Memory Budget (RP2350)

| Component | SRAM (bytes) | Flash (bytes) |
|-----------|--------------|----------------|
| NmeaParser | 200 | 0 |
| MapMatcher | 8 | route_data (~20KB) |
| KalmanFilter | 24 | 0 |
| ModeController | 48 | 0 |
| StopDetector | 256 × 16 = 4096 | 0 |
| PersistenceManager | 32 | 0 |
| Coordinator stacks, buffers | 1024 | 0 |
| LUTs (gaussian, logistic) | 0 | 384 |
| **Total** | **~5.5 KB** | **~20.5 KB** |

Well within RP2350’s 520 KB SRAM and 2 MB Flash.

---

## Key Improvements from Critique

| Issue | Fix |
|-------|-----|
| GPS outage handling | `KalmanInput::NoFix` + internal DR logic |
| Mode controller too large | Still unified but internal functions split; outputs are explicit struct |
| Coordinator complexity | Extracted `process_gps_fix` function; I/O still async |
| Event vector capacity | Replaced vectors with explicit output struct (no dynamic allocation) |
| Recovery timeout | Handled by `ModeController` via `recovering_since` and triggered recovery |
| Persistence async | Separate `PersistenceManager` task with request queue |
| StopState size unknown | Add compile-time asserts in code |

---

## Next Steps

1. **Implement each component** in its own module with unit tests.
2. **Write integration test** using pre‑recorded NMEA log and expected arrival events.
3. **Run on host** with `cargo test`.
4. **Deploy to RP2350** with Embassy async runtime.

This architecture is **simple, testable, and embedded‑ready**. No more monolithic blobs.


---

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                              HARDWARE (RP2350)                                               │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐         │
│  │     UART     │    │    FLASH     │    │    TIMER     │    │     DMA      │    │     GPIO     │         │
│  │  (GPS NMEA)  │    │ (Route Data, │    │   (SysTick)  │    │   (UART RX)  │    │  (Optional)  │         │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘    └──────┬───────┘    └──────────────┘         │
└─────────┼───────────────────┼───────────────────┼───────────────────┼──────────────────────────────────────┘
          │                   │                   │                   │
          │ UART bytes        │ Flash read/write   │ 1Hz wake?        │
          │ (async read)      │ (async erase/write)│ (not used)       │
          ▼                   ▼                   ▼                   ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                         COORDINATOR (main.rs)                                               │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│  │                                          ASYNC LOOP                                                  │   │
│  │  ┌───────────────┐   ┌───────────────┐   ┌───────────────┐   ┌───────────────┐   ┌─────────────┐   │   │
│  │  │ UART Line Buf │   │ NMEA Parser   │   │ Map Matcher   │   │ Kalman Filter │   │ Stop Detect │   │   │
│  │  │ (line buffer) │──▶│ (stateful)    │──▶│ (stateful)    │──▶│ (stateful)    │──▶│ (stateful)  │   │   │
│  │  └───────────────┘   └───────────────┘   └──────┬────────┘   └───────────────┘   └──────┬──────┘   │   │
│  │                                                  │                                      │          │   │
│  │                                                  ▼                                      ▼          │   │
│  │  ┌───────────────┐   ┌───────────────┐   ┌───────────────┐   ┌───────────────┐   ┌─────────────┐   │   │
│  │  │ Mode Control  │◀──│ Recovery (fn) │   │ Persistence   │   │ UART Writer  │   │ Event Queue │   │   │
│  │  │ (stateful)    │   │ (pure)        │   │ (simple async)│   │ (async)      │   │ (optional)  │   │   │
│  │  └───────────────┘   └───────────────┘   └───────────────┘   └───────────────┘   └─────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘

Legend:
  ──▶  synchronous data flow (function call, returns immediately)
  ──▶  asynchronous data flow (await, yields)
  ──▶  control / command flow

================================================================================================================

                               DETAILED COMPONENT INTERFACES (sync/async)

┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 1. UART READER (async)                                                                                      │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  read_nmea_sentence(uart, &mut line_buf) -> Option<&str>   // 150ms timeout, yields              │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (NMEA sentence, owned by line_buf)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 2. NMEA PARSER (sync)                                                                                       │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  struct NmeaParser { acc: FixAccumulator }                                                         │   │
│    │  fn feed_sentence(&mut self, sentence: &str) -> Option<GpsPoint>                                  │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│    Output: GpsPoint { timestamp, lat, lon, speed_cms, heading_cdeg, hdop_x10, has_fix }                     │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (GpsPoint)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 3. MAP MATCHER (sync)                                                                                       │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  struct MapMatcher { route: &RouteData, last_seg_idx: u16 }                                        │   │
│    │  fn update(&mut self, input: MapMatchInput) -> MapMatchOutput                                     │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│    Input:  MapMatchInput { gps, relax_heading, is_first_fix }                                              │
│    Output: MapMatchOutput { z_cm, divergence_d2, seg_idx }                                                 │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (MapMatchOutput)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 4. MODE CONTROLLER (sync) – first pass (may trigger recovery)                                             │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  struct ModeController { mode, counters, timers, last_stop_idx, last_valid_s, warmup }            │   │
│    │  fn update(&mut self, input: ModeControlInput, stops: &[Stop]) -> ModeControlOutput               │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│    Input:  ModeControlInput { divergence_d2, current_z, timestamp, gps_has_fix, recovery_result: None }    │
│    Output: ModeControlOutput { mode, position_for_detection, detection_enabled, relax_heading,            │
│                                trigger_recovery: Option<RecoveryParams>, snap_to_position, stop_changed } │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (if trigger_recovery is Some)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 5. RECOVERY (pure sync function)                                                                           │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  fn recover(input: RecoveryInput) -> Option<usize>                                                │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│    Input:  RecoveryInput { gps_s_cm, filtered_v_cms, dt_seconds, stops: &[Stop], hint_idx,               │
│                            frozen_s_cm, search_window }                                                  │
│    Output: Some(stop_idx) or None                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (recovery result)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 6. MODE CONTROLLER (second pass) – with recovery_result                                                   │
│    Calls update() again with recovery_result = Some(idx) → may update stop_changed, clear freeze          │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (ModeControlOutput)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 7. KALMAN FILTER (sync)                                                                                    │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  struct KalmanFilter { s_cm, v_cms, filtered_v_ema, last_fix_time }                                │   │
│    │  fn update(&mut self, input: KalmanInput, now_secs: u64) -> KalmanOutput                          │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│    Input:  KalmanInput::GpsFix { z_cm, v_cms, hdop_x10 } or NoFix { dt_seconds }                          │
│    Output: KalmanOutput { s_cm, v_cms, confidence }                                                        │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (KalmanOutput)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 8. STOP DETECTOR (sync)                                                                                    │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  struct StopDetector { stop_states: heapless::Vec<StopState, 256> }                                 │   │
│    │  fn update(&mut self, input: StopDetectorInput) -> heapless::Vec<StopEvent, 3>                    │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│    Input:  StopDetectorInput { s_cm, v_cms, timestamp }                                                   │
│    Output: Vec<StopEvent> (Announce / Arrival / Departure)                                                │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
         │ (StopEvent)
         ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 9. UART WRITER (async)                                                                                     │
│    ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│    │  write_arrival_event(uart, &event).await                                                           │   │
│    └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘

================================================================================================================

                              ASYNC DATA FLOW FOR PERSISTENCE (integrated in coordinator)

┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 10. PERSISTENCE (simple async functions, no separate task)                                                 │
│     ┌───────────────────────────────────────────────────────────────────────────────────────────────────┐   │
│     │  persist::load(flash).await -> Option<PersistedState>   // called once at boot                     │   │
│     │  persist::save(flash, &state).await -> Result<(),()>    // called when stop changes + cooldown   │   │
│     └───────────────────────────────────────────────────────────────────────────────────────────────────┘   │
│     Cooldown: u8 counter decremented each GPS tick, reset to 60 after save.                                │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘

================================================================================================================

                                          MEMORY & THREADING MODEL

   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
   │  Interrupt  │    │  Main Loop  │    │  Flash I/O  │    │   Timer     │
   │  (UART RX)  │    │ (Embassy)   │    │  (async)    │    │ (optional)  │
   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘    └──────┬──────┘
          │                  │                  │                  │
          │ Fill buffer      │ Poll FIFO        │ Blocking? No,    │
          │ Wake executor    │ (wfi)            │ yields via await │
          └──────────────────┴──────────────────┴──────────────────┘

   All components are Send + Sync (no interior mutability). Coordinator owns all state.

================================================================================================================

                                         COMPLETE DATA FLOW (one GPS tick)

   [UART] --> sentence --> NMEA Parser --> GpsPoint
                                             │
                                             ▼
                                    Map Matcher --> z_cm, divergence_d2
                                             │
                    ┌────────────────────────┼────────────────────────┐
                    │                        │                        │
                    ▼                        ▼                        ▼
            ModeController         Kalman Filter            (if trigger_recovery)
            (check off-route,           │                        │
             jump, warmup)              │                        ▼
                    │                   │                 Recovery (pure)
                    │                   │                        │
                    ▼                   ▼                        ▼
            ModeController          KalmanOut            ModeController
            (second pass)               │                 (with result)
                    │                   │                        │
                    └───────────────────┼────────────────────────┘
                                        ▼
                                 StopDetector --> events
                                        │
                                        ▼
                                 UART Writer (async)
                                        │
                                        ▼
                                   [UART TX]

   Parallel: Persistence (if stop changed & cooldown=0) --> flash.write().await

================================================================================================================

                                           LEGEND & NOTES

   ──▶  = synchronous function call (returns immediately)
   ──▶  = asynchronous call (await may yield)
   ──▶  = control flow (conditional)

   All components are implemented in pure Rust with no_std + embassy where needed.
   Synchronous components are CPU-bound but fast (<1ms per tick).
   Asynchronous components yield to executor during I/O (UART TX, flash write).
   No global state; all state lives in coordinator struct.
   Testing: each component has host unit tests (std feature) using mock route data.
```