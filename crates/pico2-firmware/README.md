# Pico 2 W Firmware

Bus arrival detection firmware for Raspberry Pi Pico 2 W.

## Architecture (v9.0 - Clear Component Boundaries)

The firmware uses a **3-layer architecture** with clear component boundaries:

### Component Layers

- **Parser Layer:** `NmeaParser` component (NMEA → GpsPoint)
- **Estimation Layer:** `EstimationState` with isolated Kalman/DR pipeline
- **Control Layer:** `SystemState` orchestrating `ModeMachine`, estimation, detection, recovery

### Key Components

- **`NmeaParser`**: `feed_sentence()` → `Option<GpsPoint>` (pure state update)
- **`ModeMachine`**: `update(ModeInput)` → `ModeOutput` (pure state machine)
- **`Estimation`**: `estimate(EstimationInput)` → `EstimationOutput` (isolated pipeline)
- **`Recovery`**: `recover(RecoveryInput)` → `Option<usize>` (pure function)

### Principles

- **Isolation:** Estimation has no access to mode/stop state
- **Single Transition:** ModeMachine enforces one transition per tick
- **Explicit Boundaries:** Each component has well-defined inputs/outputs
- **Testability:** Components can be tested in isolation

### Mode System

- **Normal:** Kalman-filtered position, arrival detection enabled
- **OffRoute:** Position frozen, detection suppressed
- **Recovering:** Raw GPS position, recovery search active

### Entry Point

`main.rs` uses `SystemState::tick(gps, est_state)` → `Option<ArrivalEvent>`

The old `state::State` is deprecated but still available for backward compatibility.

## Directory Structure

```
src/
├── main.rs           # Entry point
├── lib.rs            # Public exports
├── parser.rs         # NMEA parser component
├── control/          # Control layer
│   ├── mod.rs        # Control module exports
│   ├── machine.rs    # ModeMachine (pure state machine)
│   ├── mode.rs       # SystemMode and transition logic
│   └── recovery.rs   # Recovery function (pure)
├── estimation/       # Estimation layer (isolated)
│   ├── mod.rs        # Estimation module exports
│   ├── kalman.rs     # Kalman filter
│   └── dr.rs         # Dead-reckoning
└── state.rs          # Old State (deprecated)
```

## Building

```bash
cargo build --release --package pico2-firmware
```

## Flashing

The built UF2 file can be found at:
```
target/thumbv6m-none-eabi/release/pico2-firmware.uf2
```

Hold the BOOTSEL button on the Pico 2 W while plugging in USB, then copy the UF2 file to the mass storage device.

## Route Data

Place `route_data.bin` in `test_data/` directory. It will be embedded in the firmware at compile time.

## Memory Usage

- SRAM: ~2.5KB
- Flash: ~128KB for route data (XIP)
