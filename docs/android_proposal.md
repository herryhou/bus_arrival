# Android (Kotlin + Rust) Support Evaluation

## Current Architecture Analysis

### Core Rust pipeline (no_std compatible):
- shared/ — Semantic types (DistCm, SpeedCms, HeadCdeg, Prob8), binary format
- pipeline/gps_processor/ — Kalman filter, map matching, dead reckoning
- pipeline/detection/ — Bayesian arrival detection, state machine, recovery
- pico2-firmware/ — RP2350 embedded firmware (embassy-rp)

### Key Constraints (from 00-constraints.md):
- Integer-only arithmetic (no FPU on RP2350)
- < 8% CPU @ 150MHz, ~34KB Flash, < 1KB SRAM
- XIP (Execute-in-Place) for route data

---
## Android Integration Assessment

### ✅ Strengths for Android

1. Core logic already no_std — gps_processor and dut std. This is ideal for JNI.
2. Semantic type system — Clean FFI boundary. All types are repr(C) integers:
pub type DistCm = i32;
pub type SpeedCms = i32;
pub type HeadCdeg = i16;
pub type Prob8 = u8;
3. No runtime dependencies — No allocator requiredstack-allocated structs.
4. Binary format stable — RouteData loads from bytes, file-format compatible.

### ⚠️ Key Challenges

1. Android != Embedded — Constraints don't apply
  - Android has FPU (ARMv8+), 2GB+ RAM, multi-core
  - Integer-only optimization unnecessary here
  - Opportunity: Simplify by using f64 for probabi
2. JNI boundary design needed
  - Need to expose: Pipeline, RouteData, GpsPoint,
  - Option A: Hand-written JNI ( tedious but zero-cost)
  - Option B: jni crate + cbindgen (automated, som
  - Option C: uniffi (Mozilla's cross-language FFI, Kotlin support in beta)
3. Route data loading
  - Current: RouteData::load(&[u8]) from assets or file
  - Android: Asset manager or raw file descriptor
  - No std::fs in JNI context — need to pass bytes from Kotlin
4. GPS source integration
  - Current: NMEA sentences from serial GPS
  - Android: FusedLocationProviderClient (Google P GnssStatus
  - Kotlin layer needs to bridge Location → GpsPoint
5. Testing & validation
  - Existing tests use std (cargo test)
  - Android: Need JNI instrumentation tests or purdk

---

## Recommended Architecture

```txt
┌─────────────────────────────────────────────────────┐
│ Kotlin Layer (Android App)
│ - GPS source (FusedLocationProvider)               │
│ - Route data loader (Assets)
│ - UI/notifications                                 │
└─────────────────┬───────────────────────────────
                  │ JNI (minimal boundary)
┌─────────────────▼───────────────────────────────
│ Rust Core (static library, .so)                    │
│ ┌───────────────────────────────────────────────
│ │ shared (types, RouteData, binary format)        │ │
│ │ gps_processor (Kalman, map matching, DR)
│ │ detection (probability, FSM, recovery)          │ │
│ └───────────────────────────────────────────────
│ NO std dependency — pure no_std logic              │
└─────────────────────────────────────────────────
```

### Rust crate additions:
```txt
crates/
├── shared/           # (existing, no changes)
├── gps_processor/    # (existing, no changes)
├── detection/        # (existing, no changes)
└── jni_bridge/       # NEW: Thin JNI wrapper
    ├── src/lib.rs    # JNI entry points
    ├── Cargo.toml    # jni, ndk-glue deps
    └── build.rs      # cbindgen for headers
```

---

## Implementation Plan

### Phase 1: Rust JNI Bridge (2-3 days)

1. Create crates/jni_bridge with jni crate
2. Expose core types via #[no_mangle] extern "C":
```toml
#[no_mangle]
pub extern "C" fn bus_detector_create(route_data: t BusDetector;

#[no_mangle]
pub extern "C" fn bus_detector_feed_gps(
    detector: *mut BusDetector,
    timestamp: u64,
    lat: f64,
    lon: f64,
    heading_cdeg: i16,
    speed_cms: i32,
    hdop_x10: u16,
) -> *mut ArrivalEvent;
```
3. Use cbindgen to generate C headers for Kotlin/Jre JNI)

### Phase 2: Kotlin Integration (3-4 days)

1. Create BusDetector Kotlin class with JNI calls
2. Load route data from assets:
val routeBytes = assets.open("route_data.bin").rea
val detector = BusDetector(routeBytes)
3. Bridge Location → GPS feed:
fun onLocationChanged(location: Location) {
    val event = detector.feedGps(
        location.time,
        location.latitude,
        location.longitude,
        (location.bearing * 100).toInt(),
        (location.speed * 100).toInt(),
        0 // HDOP from GNSSStatus if available
    )
    event?.let { handleArrival(it) }
}

### Phase 3: Testing & Validation (2-3 days)

1. Port existing integration tests to Android (JUnit + JNI)
2. Use cargo-ndk for cross-compilation to ARM64, x
3. Field testing with real GPS data

---

## Estimated Effort

```txt
┌─────────────────────────┬───────────┬───────────
│        Component        │  Effort   │           Risk           │
├─────────────────────────┼───────────┼───────────
│ Rust JNI bridge         │ 2-3 days  │ Low — pure wrapper       │
├─────────────────────────┼───────────┼───────────
│ Kotlin integration      │ 3-4 days  │ Low — straightforward    │
├─────────────────────────┼───────────┼───────────
│ Cross-compilation setup │ 1 day     │ Low — cargo-ndk mature   │
├─────────────────────────┼───────────┼───────────
│ Testing/validation      │ 2-3 days  │ Medium — GPS variability │
├─────────────────────────┼───────────┼───────────
│ Total                   │ 8-11 days │ Low-Medium               │
└─────────────────────────┴───────────┴───────────
```

---

## Decision Points

1. Use f64 on Android?
  - Yes — Android has FPU, simplifies probability
  - Keep integer-only for embedded only
2. JNI vs UniFFI?
  - JNI for speed, control, mature tooling
  - UniFFI if targeting multiple platforms (iOS +
3. GPS source?
  - FusedLocationProvider — Battery-optimized, but
  - GnssStatus + raw NMEA — Full control, matches embedded behavior
4. Route data distribution?
  - Assets — Bundle with APK
  - Network — Download at runtime (supports route

---
## Recommendation

Proceed with Android support. The architecture is well-suited:

1. Core detection logic is already no_std — minimal refactoring
2. Semantic types provide clean FFI boundary
3. Binary format compatible — route data portable
4. JNI approach proven, low risk

Suggested first step: Create crates/jni_bridge andon bus_detector_create() to validate build chain.

✻ Cogitated for 52s

===

# Host + Android Shared Architecture (No Embedded Constraints)
Dropping embedded constraints simplifies dramatically. Single std codebase, JNI boundary for Android only.

## Adjusted Architecture

```txt
┌─────────────────────────────────────────────────────────────┐
│                    Host (CLI/Server)
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Cargo binary: `bus-arrival-host`
│ │ - NMEA file processing                                   │ │
│ │ - Route preprocessing
│ │ - Validation tools                                       │ │
│ └───────────────────────────────────────────────
└─────────────────────────────────────────────────────────────┘
                            │
                            │ Shared Rust Core (std + f64)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                  Rust Core Library
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ bus-arrival-core (lib)
│ │ ├── types/       (now using f64, not semantic ints)    │ │
│ │ ├── kalman/      (f64-based Kalman filter)
│ │ ├── map_match/   (f64 distance calculations)            │ │
│ │ ├── detection/   (f64 probability model)
│ │ ├── route/       (RouteData, loading)                   │ │
│ │ └── nmea/        (NMEA parsing)
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────
                            │
                            │ JNI
                            ▼
┌─────────────────────────────────────────────────
│                    Android (Kotlin)                         │
│ ┌───────────────────────────────────────────────
│ │ BusDetector.kt (JNI wrapper)                            │ │
│ │ - Feed GPS from FusedLocationProvider
│ │ - Receive arrival/departure events                      │ │
│ │ - UI/notifications
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────
```

## Key Changes from Embedded

```txt
┌───────────────┬──────────────────────────────┬───┐
│   Component   │      Before (Embedded)       │      After (Host+Android)      │
├───────────────┼──────────────────────────────┼───┤
│ Types         │ DistCm = i32, SpeedCms = i32 │ f64 for distance/speed         │
├───────────────┼──────────────────────────────┼───┤
│ Kalman        │ Integer gain (51/256)        │ Standard f64 Kalman            │
├───────────────┼──────────────────────────────┼───┤
│ Probability   │ LUT-based (u8[256])          │ Gaussian f64 exp()             │
├───────────────┼──────────────────────────────┼───┤
│ Route data    │ XIP Flash, raw bytes         │ Heap-allocated structs         │
├───────────────┼──────────────────────────────┼───┤
│ Distance      │ Dot product with i64         │ haversine() f64                │
├───────────────┼──────────────────────────────┼───┤
│ Binary format │ v5.1 sparse grid             │ Optional, can use JSON/MsgPack │
└───────────────┴──────────────────────────────┴───┘
```

---

## Simplified Crate Structure
```txt
bus-arrival/
├── Cargo.toml                 # Workspace
├── crates/
│   ├── core/                  # NEW: Shared std l
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types.rs       # f64-based types
│   │   │   ├── kalman.rs      # f64 Kalman
│   │   │   ├── map_match.rs   # f64 map matching
│   │   │   ├── detection.rs   # f64 probability
│   │   │   ├── route.rs       # RouteData (serde)
│   │   │   └── nmea.rs        # NMEA parser
│   │   └── Cargo.toml         # std, serde, nalgebra
│   │
│   ├── host-cli/              # NEW: Host CLI binary
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   │
│   ├── android/               # NEW: JNI library
│   │   ├── src/lib.rs         # JNI entry points
│   │   ├── Cargo.toml         # jni, ndk-glue
│   │   └── build.rs           # cbindgen
│   │
│   └── preprocessor/          # (optional, for ro
│
├── android/                   # Kotlin project
│   ├── app/src/main/java/.../BusDetector.kt
│   └── build.gradle.kts
│
└── README.md
```

---

## Core API Design (f64-based)

```rust
types.rs — Simplified types:

/// Distance in meters (f64)
pub type Distance = f64;

/// Speed in m/s (f64)
pub type Speed = f64;

/// Heading in degrees (0-360)
pub type Heading = f64;

/// Probability (0.0 - 1.0)
pub type Probability = f64;

/// GPS point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsPoint {
    pub timestamp: u64,
    pub lat: f64,
    pub lon: f64,
    pub heading: Option<Heading>,
    pub speed: Option<Speed>,
    pub hdop: Option<f64>,
}

kalman.rs — Standard f64 Kalman:

pub struct KalmanFilter {
    pub x: f64,    // Position estimate
    pub v: f64,    // Velocity estimate
    pub p: f64,    // Position variance
    pub q: f64,    // Process noise
    pub r: f64,    // Measurement noise
}

impl KalmanFilter {
    pub fn new() -> Self {
        Self {
            x: 0.0, v: 0.0,
            p: 1000.0, q: 0.1, r: 25.0,
        }
    }

    pub fn update(&mut self, z: f64, dt: f64) -> (
        // Predict
        self.x += self.v * dt;
        self.p += self.q;

        // Update
        let k = self.p / (self.p + self.r);
        self.x += k * (z - self.x);
        self.v += k * (z - self.x - self.v * dt);
        self.p *= (1.0 - k);

        (self.x, self.v)
    }
}

detection.rs — Gaussian probability:

pub fn arrival_probability(
    dist_to_stop: Distance,
    speed: Speed,
    progress_error: Distance,
    dwell_time: f64,
) -> Probability {
    // F1: Distance likelihood (Gaussian)
    let p_d = gaussian_pdf(dist_to_stop, 0.0, SIGM

    // F2: Speed likelihood (exponential decay nea
    let p_v = if speed < 2.0 { 1.0 } else { (-0.5 * speed).exp() };

    // F3: Progress error likelihood
    let p_p = gaussian_pdf(progress_error, 0.0, SI

    // F4: Dwell time likelihood
    let p_t = if dwell_time > 5.0 { 1.0 } else { dwell_time / 5.0 };

    // Weighted combination
    0.3 * p_d + 0.2 * p_v + 0.3 * p_p + 0.2 * p_t
}

fn gaussian_pdf(x: f64, mu: f64, sigma: f64) -> f64 {
    let diff = x - mu;
    (-0.5 * (diff / sigma).powi(2)).exp()
}

---
JNI Bridge (android/src/lib.rs)

use jni::JNIEnv;
use jni::objects::{JClass, JObject};
use jni::sys::{jlong, jdouble, jint};
use bus_arrival_core::{BusDetector, GpsPoint};

#[no_mangle]
pub extern "C" fn Java_com_busarrival_BusDetector_create(
    env: JNIEnv,
    _class: JClass,
    route_data: JObject,
) -> jlong {
    let detector = Box::new(BusDetector::new());
    Box::into_raw(detector) as jlong
}

#[no_mangle]
pub extern "C" fn Java_com_busarrival_BusDetector_feedGps(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    timestamp: jlong,
    lat: jdouble,
    lon: jdouble,
    heading: jdouble,
    speed: jdouble,
    hdop: jdouble,
) -> jobject {
    let detector = unsafe { &mut *(ptr as *mut Bus

    let gps = GpsPoint {
        timestamp: timestamp as u64,
        lat, lon,
        heading: if heading >= 0.0 { Some(heading) } else { None },
        speed: if speed >= 0.0 { Some(speed) } els
        hdop: if hdop > 0.0 { Some(hdop) } else { None },
    };

    if let Some(event) = detector.feed_gps(gps) {
        // Convert ArrivalEvent to Kotlin object
        create_arrival_event_object(env, event)
    } else {
        std::ptr::null_mut()
    }
}

---
Kotlin Wrapper (BusDetector.kt)

class BusDetector private constructor(private val nativePtr: Long) {

    companion object {
        init {
            System.loadLibrary("bus_arrival_android")
        }

        fun create(routeData: ByteArray): BusDetec
            val ptr = createNative(routeData)
            return BusDetector(ptr)
        }

        private external fun createNative(routeData: ByteArray): Long
        private external fun feedGpsNative(
            ptr: Long,
            timestamp: Long,
            lat: Double,
            lon: Double,
            heading: Double,
            speed: Double,
            hdop: Double
        ): ArrivalEvent?

        private external fun destroy(ptr: Long)
    }

    fun feedGps(location: Location): ArrivalEvent? {
        return feedGpsNative(
            nativePtr,
            location.time,
            location.latitude,
            location.longitude,
            location.bearing,
            location.speed,
            0.0 // HDOP from GnssStatus if available
        )
    }

    fun close() = destroy(nativePtr)
}

data class ArrivalEvent(
    val timestamp: Long,
    val stopIndex: Int,
    val distance: Double,
    val speed: Double,
    val probability: Double
)
```

---

## Migration Strategy

Phase 1: Create core crate (1 week)
1. New bus-arrival-core with std, f64 types
2. Port Kalman, detection, map matching to f64
3. Unit tests with cargo test

Phase 2: Host CLI (3 days)
1. host-cli binary using core
2. Replace existing pipeline crate
3. Integration tests with existing NMEA files

Phase 3: Android JNI (1 week)
1. android crate with JNI bridge
2. Kotlin wrapper + Gradle setup
3. Cross-compile with cargo-ndk

Phase 4: Deprecate embedded (optional)
1. Keep pico2-firmware as separate project
2. Or remove entirely if embedded abandoned

---

## Binary Format Decision

Option A: Keep binary format (fast, compact)
- Pros: Efficient loading, compatible with existin
- Cons: Still need serialization layer

Option B: Use serde-compatible format (JSON/MsgPack)
- Pros: Simpler, cross-language, debuggable
- Cons: Slower, larger files

Recommendation: Keep binary format for efficiency, but add JSON export/import for debugging.

---

## Estimated Effort

```txt
┌───────────────────────┬────────────┬────────────
│         Phase         │   Effort   │           Notes           │
├───────────────────────┼────────────┼────────────
│ Core crate (f64 port) │ 5-7 days   │ Straightforward math port │
├───────────────────────┼────────────┼────────────
│ Host CLI              │ 2-3 days   │ Simple binary wrapper     │
├───────────────────────┼────────────┼────────────
│ Android JNI           │ 5-7 days   │ JNI + Kotlin + Gradle     │
├───────────────────────┼────────────┼────────────
│ Testing               │ 3-4 days   │ Port existing tests       │
├───────────────────────┼────────────┼────────────
│ Total                 │ 15-21 days │ ~3 weeks                  │
└───────────────────────┴────────────┴────────────
```

---

## Verdict

Host + Android shared architecture is cleaner and simpler. Dropping embedded constraints eliminates most complexity:

- Single std codebase
- f64 everywhere (simpler math)
- Serde for serialization (no binary format quirks)
- JNI as only cross-language boundary

