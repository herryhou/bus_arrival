# Pico 2 W Firmware no_std Migration Design

**Date:** 2026-04-03  
**Status:** Design Approved  
**Target:** Embassy-rp async firmware  
**Goal:** Achieve true no_std build by migrating from rp2040-hal to embassy-rp

---

## Problem Statement

The Pico 2 W firmware cannot build in true no_std mode due to transitive dependencies from `rp2040-hal`:

1. **`memchr`** - From `pio-proc` → `lalrpop-util` → `regex-automata` → `aho-corasick` → `memchr`  
   - Has `extern crate std;` requiring std

2. **`serde_core`** - From `heapless` → `usb-device` → `rp2040-hal`  
   - Lacks `#![no_std]` declaration

**Already Completed (Previous Work):**
- ✅ Replaced `crc32fast` with pure Rust CRC32 implementation
- ✅ Replaced `serde_json_core` with manual JSON serialization
- ✅ Made `BusError` traits conditional on std

**Remaining Work:** Replace `rp2040-hal` with `embassy-rp` for true no_std support.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                   Main (Entry Point)                    │
│  • Clocks, GPIO init (once)                            │
│  • Embassy executor (static allocation)                │
│  • Spawn main GPS task                                 │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              Embassy Executor (static)                  │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Task: gps_pipeline (async, runs forever)        │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │ 1. Read NMEA (async UART)                  │  │  │
│  │  │ 2. Parse GPS → GpsPoint                    │  │  │
│  │  │ 3. Kalman filter update                     │  │  │
│  │  │ 4. Update stop state machines                │  │  │
│  │  │ 5. Emit events (async UART) if triggered     │  │  │
│  │  │ 6. Timer::after(1 sec).await loop           │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│            Embassy RP HAL Layer                         │
│  • UART driver (async)                                  │
│  • Time driver (Timer, Delay)                            │
│  • Interrupt controller                                  │
│  • GPIO (for future LED indicators, etc.)               │
└─────────────────────────────────────────────────────────┘
```

---

## Component Structure

**File Structure:**
```
crates/pico2-firmware/src/
├── main.rs           # Entry point, executor setup
├── gps_pipeline.rs   # Async GPS processing task
├── uart.rs           # UART wrappers (adapt for async)
├── interrupt.rs      # Interrupt handlers (future expansion)
└── executor.rs       # Static executor configuration
```

**Component Responsibilities:**

| Component | Responsibility |
|-----------|---------------|
| `main.rs` | Clock initialization, Embassy executor setup, Spawn GPS pipeline task, Static buffer definitions |
| `gps_pipeline.rs` | Async GPS processing loop, Kalman filter + stop state machines, Event emission |
| `uart.rs` | Async UART driver wrapper, Buffered I/O, NMEA framing (keep existing) |
| `executor.rs` | Static executor type definition, Task allocation configuration |
| `interrupt.rs` | Embassy interrupt handling (future expansion) |

---

## Data Flow (Async)

```rust
async fn gps_pipeline_task(
    uart: &mut UartRx<'static, UART0>,
    uart_tx: &mut UartTx<'static, UART0>,
    route_data: &'static RouteData<'static>,
    state: &mut State,
) -> ! {
    loop {
        // 1. Async GPS read (non-blocking)
        let sentence = read_nmea_line_async(uart).await?;
        
        // 2. Parse GPS (sync, fast)
        if let Some(gps) = state.nmea.parse_sentence(sentence) {
            // 3. Kalman + Detection (sync, fast)
            update_state(gps, route_data, state);
            
            // 4. Emit events if triggered
            if let Some(event) = state.pop_event() {
                emit_event_async(uart_tx, event).await?;
            }
        }
        
        // 5. Rate limiting (async delay, not busy-wait)
        Timer::after(Duration::from_secs(1)).await;
    }
}
```

**Key Async Transformations:**

| Before (rp2040-hal) | After (embassy-rp) |
|---------------------|-------------------|
| `block!(uart.read())` | `uart.read_byte().await` |
| `loop { busy_wait }` | `loop { Timer::after().await }` |
| Fixed-size buffers | Static buffers + no alloc |
| Sync main loop | Async task in executor |

**Buffer Strategy (No Heap):**
```rust
static UART_RX_BUF: Buffer<256> = Buffer::new();
static UART_TX_BUF: Buffer<128> = Buffer::new();
static EXECUTOR_BUFFER: [u8; 4096] = [0; 4096]; // For executor
```

---

## Error Handling

**Error Types (no_std compatible):**
```rust
#[derive(Debug)]
enum FirmwareError {
    UartRead,
    UartWrite,
    GpsParse,
    InvalidData,
    BufferOverflow,
}

type Result<T> = core::result::Result<T, FirmwareError>;
```

**Error Handling Strategy:**
- **GPS Read Errors:** Log via defmt, skip cycle, continue
- **GPS Parse Errors:** Ignore sentence, continue (GPS sends 1Hz)
- **UART Write Errors:** Log, drop event (GPS will re-trigger), continue
- **Buffer Overflow:** Reset buffer, log warning, continue

**No Panic Philosophy:**
- Use `?` with graceful degradation
- Prefer `if let Some()` over `unwrap()`
- No `println!` or `eprintln!` (requires std)

---

## Dependencies

**Updated Cargo.toml:**
```toml
[features]
default = []
dev = ["shared/std", "gps_processor/std", "detection/std", "defmt"]

[dependencies]
shared = { path = "../shared", default-features = false }
gps_processor = { path = "../pipeline/gps_processor", default-features = false }
detection = { path = "../pipeline/detection", default-features = false }

# Embassy RP (replaces rp2040-hal)
embassy-rp = { version = "0.3", features = ["rp2350", "time-driver", "defmt", "internal-irqs"] }
embassy-executor = { version = "0.7", features = ["arch-cortex-m", "executor-thread"] }
embassy-time = { version = "0.4", features = ["defmt", "defmt-timestamp-uptime"] }
embassy-embedded-hal = "0.3"

# Core embedded
cortex-m = "0.7"
embedded-hal = "1.0"
embedded-hal-nb = "1.0"
nb = "1.0"
panic-halt = "1.0"
defmt = "0.3"
```

**Removed Dependencies:**
- `rp2040-hal`
- `rp2040-boot2`
- `cortex-m-rt`
- `serde-json-core`
- `heapless`

---

## Testing Strategy

| Layer | Tool | Coverage |
|-------|------|----------|
| Unit Tests | `cargo test` (host) | GPS parsing, Kalman filter, detection logic |
| Integration Tests | `cargo test` (host) | Full pipeline mock GPS |
| Hardware Tests | Flash to Pico 2 W | Real GPS module, verify UART output |
| No_std Verification | `cargo build --no-default-features --target thumbv8m.main-none-eabi` | Confirm clean compile |

**Hardware Test Plan:**
1. Flash firmware to Pico 2 W
2. Connect GPS module (UART0 GPIO 0/1)
3. Monitor output via second UART or USB
4. Verify: JSON events emitted, no panics, 1 Hz processing
5. Test scenarios: normal GPS, GPS jump, GPS outage

---

## Migration Steps

1. **Update dependencies** (1 hour) - Remove rp2040-hal, add embassy-rp
2. **Add executor setup** (2 hours) - Create executor.rs, update main.rs
3. **Migrate UART driver** (3 hours) - Replace rp2040 UART with embassy async UART
4. **Convert main loop to async task** (2 hours) - Create gps_pipeline.rs
5. **Update memory.x and linker script** (1 hour) - Adjust for static executor
6. **Add defmt logging** (2 hours) - Replace debug prints with defmt
7. **Testing and validation** (4+ hours) - Build, flash, verify on hardware

**Total Estimated Time:** 15-20 hours

---

## Success Criteria

✅ **Build Success:** `cargo build --release --no-default-features --target thumbv8m.main-none-eabi` completes without errors  
✅ **No Std Dependencies:** Dependency tree contains no serde, memchr, or other std-requiring crates  
✅ **Hardware Verification:** Firmware runs on Pico 2 W with GPS module, emits JSON events  
✅ **Functionality Parity:** Same GPS processing behavior as rp2040-hal version  
✅ **Clean Code:** No unsafe workarounds, follows embassy best practices

---

## Future Considerations

- **WiFi/Networking:** Embassy-rp supports async WiFi for future expansion
- **Additional Interrupts:** embassy-rp interrupt handling for GPIO, timers
- **Power Management:** Embassy-rp provides low-power sleep modes
- **Multi-core:** Embassy can utilize both cores on RP2350 (if needed)
