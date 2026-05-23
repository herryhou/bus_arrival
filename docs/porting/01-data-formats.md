# Data Formats Specification

## NMEA Input Format

### Required Sentences

**$GPRMC / $GNRMC** - Recommended Minimum data

```
$GPRMC,hhmmss.ss,A,ddmm.mmmm,N,dddmm.mmmm,E,sss.s,ddd.d,ddmmyy,,,D*hh<CR><LF>
```

| Field | Position | Type | Description |
|-------|----------|------|-------------|
| Time | 1 | string | UTC timestamp hhmmss.ss |
| Status | 2 | char | 'A' = valid, 'V' = warning |
| Lat | 3 | ddmm.mmmm | Latitude ddmm.mmmm |
| N/S | 4 | char | 'N' or 'S' |
| Lon | 5 | dddmm.mmmm | Longitude dddmm.mmmm |
| E/W | 6 | char | 'E' or 'W' |
| Speed_knots | 7 | float | Speed in knots |
| Heading | 8 | float | True heading in degrees |
| Date | 9 | ddmmyy | Date ddmmyy |

**Conversion:**
- Speed: cm/s = knots × 51.44
- Heading: cdeg = degrees × 100

**$GPGGA / $GNGGA** - Fix Data

```
$GPGGA,hhmmss.ss,llll.ll,a,yyyyy.yy,a,x,xx,x.x,x.x,M,x.x,M,x.x,xxxx*hh<CR><LF>
```

| Field | Position | Type | Description |
|-------|----------|------|-------------|
| Time | 1 | string | UTC timestamp |
| Lat | 2 | llll.ll | Latitude |
| N/S | 3 | char | 'N' or 'S' |
| Lon | 4 | yyyy.yy | Longitude |
| E/W | 5 | char | 'E' or 'W' |
| Quality | 6 | int | 0=invalid, 1=GPS, 2=DGPS |
| Sats | 7 | int | Number of satellites |
| HDOP | 8 | float | Horizontal dilution of precision |
| Alt | 9 | float | Altitude above sea level |

**HDOP interpretation:**
- ≤ 2.0: Excellent (K_s = 77/256)
- 2.1-3.0: Good (K_s = 51/256)
- 3.1-5.0: Fair (K_s = 26/256)
- > 5.0: Poor (K_s = 13/256)

## Core Data Types

Source: `crates/shared/src/lib.rs`

```rust
pub type DistCm   = i32;  // Distance in centimeters (±214 km)
pub type SpeedCms = i32;  // Speed in cm/s (0..214 km/h)
pub type HeadCdeg = i16;  // Heading in 0.01° (-180°..+180°)
pub type GeoCdeg  = i16;  // Lat/lon in 0.01°
pub type Prob8    = u8;   // Probability × 255 (0..255)
pub type Dist2    = i64;  // Distance² (cm²)
```

| Type | Unit | Range | Purpose |
|------|------|-------|---------|
| DistCm | cm | ±21,474,836 cm | All distances |
| SpeedCms | cm/s | 0..21,474,836 | Speed calculations |
| HeadCdeg | 0.01° | -18000..18000 | Heading/direction |
| GeoCdeg | 0.01° | -18000..18000 | GPS coordinates |
| Prob8 | 1/256 | 0..255 | Probability values |
| Dist2 | cm² | ±4.6×10¹⁸ | Intermediate distance calc |

## Binary Route Format

### Version 5 Structure (v8.8)

```
[Header: 16 bytes]
  magic: "PICO2RT" (8 bytes)
  version: u32 = 5
  node_count: u32
  stop_count: u32

[Grid Origin: 8 bytes]
  x0_cm: i32  (min x of route)
  y0_cm: i32  (min y of route)

[Spatial Grid: variable size]
  grid_width: u16
  grid_height: u16
  bitmask: [u8] (ceil(width × height / 8))
  cell_offsets: [u16] (non-empty cell count)
  cell_data: [u16] (segment indices per cell)

[Route Nodes: node_count × 24 bytes]
  For each node (repr(C), 24 bytes):
    x_cm: i32          (0)
    y_cm: i32          (4)
    cum_dist_cm: i32   (8)
    seg_len_mm: i32    (12)
    dx_cm: i16         (16)
    dy_cm: i16         (18)
    heading_cdeg: i16  (20)
    _pad: i16          (22)

[Stops: stop_count × 16 bytes]
  For each stop (repr(C), 16 bytes):
    index: u8              (0)
    s_cm: i32              (1, padded to 4)
    corridor_start_cm: i32 (5)
    corridor_end_cm: i32   (9)
    _pad: [u8; 3]         (13)
```

### Memory Layout Example

```
Offset  | Size    | Content
--------|---------|------------------------
0       | 16      | Header
16      | 8       | Grid Origin
24      | 2       | Grid Width
26      | 2       | Grid Height
28      | ~450    | Bitmask (for 60×60 grid)
478     | ~1.5K   | Cell Offsets
~2K     | ~4K     | Cell Data
~6K     | ~15K    | Route Nodes (600 × 24)
~21K    | ~1K     | Stops (40 × 16)
~22K    |         | Total
```

### XIP Considerations
- All structures are repr(C) for Flash access
- No pointer indirection (pure offsets)
- Alignment: 4-byte for i32, 2-byte for i16

## trace_v2.jsonl Output Format

### Per-Tick JSON Line

Each line is a complete JSON object representing one GPS tick (1 Hz):

```json
{
  "time": 12345,
  "gps_state": {
    "x_cm": 27564320,
    "y_cm": 27678910,
    "heading_cdeg": 2750,
    "speed_cms": 555,
    "hdop_x10": 18,
    "fix_quality": 1
  },
  "kalman_state": {
    "s_cm": 1234567,
    "v_cms": 520,
    "z_cm": 1234500
  },
  "detection_state": {
    "stop_states": [
      {
        "index": 5,
        "fsm_state": "Approaching",
        "d_to_stop_cm": 6500,
        "probability": 128,
        "dwell_time_s": 3
      }
    ],
    "active_stop_count": 1
  }
}
```

### Field Definitions

**gps_state:**
- `time`: Unix timestamp or tick counter
- `x_cm`, `y_cm`: Projected local coordinates (cm)
- `heading_cdeg`: GPS heading × 100
- `speed_cms`: GPS speed (cm/s)
- `hdop_x10`: HDOP × 10 (180 = HDOP 18.0)
- `fix_quality`: 0=none, 1=GPS, 2=DGPS

**kalman_state:**
- `s_cm`: Filtered route progress (cm)
- `v_cms`: Filtered speed (cm/s)
- `z_cm`: Raw GPS projection (cm)

**detection_state.stop_states[]:**
- `index`: Stop index (0-based)
- `fsm_state`: "Idle" | "Approaching" | "Arriving" | "AtStop" | "Departed"
- `d_to_stop_cm`: |s_hat - s_stop| (cm)
- `probability`: Arrival probability (0-255)
- `dwell_time_s`: Time in "AtStop" state (seconds)

## Ground Truth JSON Format

For validation testing:

```json
{
  "route_name": "ty225",
  "stops": [
    {
      "stop_idx": 0,
      "arrivals": [
        {
          "time": 12345,
          "dwell_s": 15
        }
      ]
    },
    {
      "stop_idx": 5,
      "arrivals": [
        {
          "time": 23456,
          "dwell_s": 20
        },
        {
          "time": 56789,
          "dwell_s": 8
        }
      ]
    }
  ]
}
```

### Validation Rules
- `time`: First tick where stop enters "AtStop" state
- `dwell_s`: Duration in "AtStop" before "Departed"
- Multiple arrivals possible per stop (detours, loops)
