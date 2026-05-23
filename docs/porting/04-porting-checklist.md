# Platform Porting Checklist

## iOS Porting

### CoreLocation Integration

**Required:**
- [ ] Configure `CLLocationManager` with 1 Hz updates
- [ ] Request `Always` location permission (background operation)
- [ ] Enable `allowsBackgroundLocationUpdates`
- [ ] Set `desiredAccuracy = kCLLocationAccuracyBestForNavigation`

**NMEA Emulation:**
iOS doesn't provide raw NMEA. Create adapter:

```swift
extension CLLocation {
    func toNMEA() -> String {
        let time = formatter.string(from: timestamp)
        let lat = abs(coordinate.latitude)
        let latDeg = Int(lat)
        let latMin = (lat - Double(latDeg)) * 60
        let latHem = coordinate.latitude >= 0 ? "N" : "S"

        let lon = abs(coordinate.longitude)
        let lonDeg = Int(lon)
        let lonMin = (lon - Double(lonDeg)) * 60
        let lonHem = coordinate.longitude >= 0 ? "E" : "W"

        let speedKnots = speed * 1.94384
        let heading = course >= 0 ? course : 0

        return "$GPRMC,\(time),A,"
             + "\(latDeg)\(String(format: "%.4f", latMin)),\(latHem),"
             + "\(lonDeg)\(String(format: "%.4f", lonMin)),\(lonHem),"
             + "\(String(format: "%.1f", speedKnots)),\(heading),,"
             + ",,,D"
    }
}
```

### Math Implementation

**Floating-point is acceptable:**
- Use `Double` for distances (meters)
- Convert to cm for integer compatibility: `let d_cm = Int32(d_m * 100)`
- Kalman filter can use full covariance matrix

**Example:**
```swift
struct KalmanState {
    var s_cm: Int32
    var v_cms: Int32
    var P: [[Double]]  // 2×2 covariance

    mutating func update(z_cm: Int32, v_gps: Int32) {
        // Predict
        let s_pred = s_cm + v_cms
        let v_pred = v_cms

        // Update (with full covariance)
        let K = computeKalmanGain(P: P)
        s_cm = Int32(Double(s_pred) + K[0] * Double(z_cm - s_pred))
        v_cms = max(0, Int32(Double(v_pred) + K[1] * Double(v_gps - v_pred)))
    }
}
```

### Threading Model

**Option A: OperationQueue**
```swift
let queue = OperationQueue()
queue.maxConcurrentOperationCount = 1
queue.qualityOfService = .userInitiated

func processGPS(location: CLLocation) {
    queue.addOperation {
        self.pipeline.tick(location.toNMEA())
    }
}
```

**Option B: Async/Await**
```swift
actor PipelineActor {
    var pipeline: BusArrivalPipeline

    func tick(_ nmea: String) async {
        await pipeline.process(nmea)
    }
}
```

### Memory Management

- Route data: Load from JSON, convert to Swift structs
- LUTs: Precompute as `[UInt8]` arrays
- Runtime state: < 1 KB (single struct instance)

## ESP32 Porting

### GPS Driver Integration

**UART Configuration:**
```c
#define GPS_UART_NUM      UART_NUM_1
#define GPS_TX_PIN        4
#define GPS_RX_PIN        5
#define GPS_BAUD_RATE     9600

void gps_init() {
    uart_config_t uart_config = {
        .baud_rate = GPS_BAUD_RATE,
        .data_bits = UART_DATA_8_BITS,
        .parity = UART_PARITY_DISABLE,
        .stop_bits = UART_STOP_BITS_1,
        .flow_ctrl = UART_HW_FLOWCTRL_DISABLE,
    };
    uart_param_config(GPS_UART_NUM, &uart_config);
    uart_set_pin(GPS_UART_NUM, GPS_TX_PIN, GPS_RX_PIN, ...);
}
```

**NMEA Parsing:**
```c
char nmea_buffer[256];

void gps_task(void* arg) {
    while (1) {
        int len = uart_read_bytes(GPS_UART_NUM, nmea_buffer, sizeof(nmea_buffer), 100 / portTICK_PERIOD_MS);
        if (len > 0 && validate_nmea(nmea_buffer, len)) {
            pipeline_tick(nmea_buffer);
        }
    }
}
```

### Memory Constraints

**SRAM Budget (520 KB total):**
- Route data: ~12 KB (load from Flash to RAM for speed)
- Runtime state: ~1 KB
- Stack per task: ~4 KB
- Free for application: > 500 KB

**Flash Storage:**
- Store route_data.bin in SPIFFS or raw Flash partition
- Use XIP (Execute-in-Place) for read-only access

**Example:**
```c
#define ROUTE_FLASH_ADDR 0x300000  // 3MB offset

const RouteNode* load_route_nodes(size_t* count) {
    const RouteHeader* header = (const RouteHeader*)ROUTE_FLASH_ADDR;
    *count = header->node_count;
    return (const RouteNode*)(ROUTE_FLASH_ADDR + sizeof(RouteHeader));
}
```

### FreeRTOS Task Structure

```c
void pipeline_task(void* arg) {
    PipelineState pipeline;
    pipeline_init(&pipeline);

    TickType_t last_tick = xTaskGetTickCount();

    while (1) {
        // Wait for GPS data (queue-based)
        NMEAMessage msg;
        if (xQueueReceive(gps_queue, &msg, pdMS_TO_TICKS(1000)) == pdTRUE) {
            pipeline_tick(&pipeline, &msg);

            // Emit trace output (optional)
            trace_emit(&pipeline);
        }

        // 1 Hz tick handling
        vTaskDelayUntil(&last_tick, pdMS_TO_TICKS(1000));
    }
}

void app_main() {
    // Create GPS task
    xTaskCreate(gps_task, "gps", 4096, NULL, 5, NULL);

    // Create pipeline task
    xTaskCreate(pipeline_task, "pipeline", 8192, NULL, 4, NULL);
}
```

### Integer Math Verification

**All runtime calculations must be integer-only:**

```c
// ✅ Correct: integer arithmetic
int32_t dist2 = dx*dx + dy*dy;

// ❌ Wrong: floating-point
float dist2 = sqrtf(dx*dx + dy*dy);  // Don't do this!

// ✅ Correct: fixed-point Kalman
int32_t s_new = s_pred + (51 * (z_gps - s_pred)) / 256;

// ❌ Wrong: floating-point Kalman
float s_new = s_pred + 0.2f * (z_gps - s_pred);  // Don't do this!
```

**LUT Usage:**
```c
// Precomputed Gaussian LUT (generated offline)
extern const uint8_t GAUSSIAN_LUT[256];

uint8_t gaussian_lut(int32_t d_cm, int32_t sigma_cm) {
    if (sigma_cm == 0) return 255;
    int32_t idx = ((int64_t)d_cm * 64) / sigma_cm;
    if (idx < 0) idx = -idx;
    if (idx >= 256) idx = 255;
    return GAUSSIAN_LUT[idx];
}
```

## Common Implementation Tasks

### NMEA Parser

**Required fields to extract:**

| Field | NMEA Source | Type | Conversion |
|-------|-------------|------|------------|
| Time | $GPRMC field 1 | string | Parse hhmmss.ss |
| Lat | $GPRMC field 3 | ddmm.mmmm | Convert to GeoCdeg |
| Lon | $GPRMC field 5 | dddmm.mmmm | Convert to GeoCdeg |
| Speed | $GPRMC field 7 | knots | × 51.44 → SpeedCms |
| Heading | $GPRMC field 8 | degrees | × 100 → HeadCdeg |
| HDOP | $GPGGA field 8 | float | × 10 → hdop_x10 |

**Validation:**
- Checksum verification (required)
- Status 'A' (valid data)
- Fix quality > 0

### Route Data Loading

**Binary format (recommended):**
```c
// C header for binary format
typedef struct __attribute__((packed)) {
    int32_t x_cm;
    int32_t y_cm;
    int32_t cum_dist_cm;
    int32_t seg_len_mm;
    int16_t dx_cm;
    int16_t dy_cm;
    int16_t heading_cdeg;
    int16_t _pad;
} RouteNode;  // 24 bytes

typedef struct __attribute__((packed)) {
    uint8_t index;
    int32_t s_cm;
    int32_t corridor_start_cm;
    int32_t corridor_end_cm;
    uint8_t _pad[3];
} Stop;  // 16 bytes
```

**JSON format (easier debugging):**
```swift
struct Route: Codable {
    let nodes: [RouteNode]
    let stops: [Stop]
    let grid: SpatialGrid
}
```

### Trace Output Generation

**Format:** One JSON line per tick

```swift
func emitTrace(_ state: PipelineState) -> String {
    let gps = [
        "time": state.time,
        "x_cm": state.gps.x_cm,
        "y_cm": state.gps.y_cm,
        "heading_cdeg": state.gps.heading_cdeg,
        "speed_cms": state.gps.speed_cms,
        "hdop_x10": state.gps.hdop_x10
    ]

    let kalman = [
        "s_cm": state.kalman.s_cm,
        "v_cms": state.kalman.v_cms,
        "z_cm": state.kalman.z_cm
    ]

    // ... build full JSON object
    return jsonString + "\n"
}
```
