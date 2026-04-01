# Pico 2 W Firmware

Bus arrival detection firmware for Raspberry Pi Pico 2 W.

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
