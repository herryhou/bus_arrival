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
private let formatter: DateFormatter = {
    let fmt = DateFormatter()
    fmt.dateFormat = "HHmmss.SS"
    fmt.timeZone = TimeZone(secondsFromGMT: 0)!
    return fmt
}()

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

        let sentence = "$GPRMC,\(time),A,"
                     + "\(latDeg)\(String(format: "%.4f", latMin)),\(latHem),"
                     + "\(lonDeg)\(String(format: "%.4f", lonMin)),\(lonHem),"
                     + "\(String(format: "%.1f", speedKnots)),\(heading),,"
                     + ",,,D"
        return sentence + computeChecksum(sentence)
    }

    private func computeChecksum(_ sentence: String) -> String {
        let data = sentence.data(using: .utf8)!
        var checksum: UInt8 = 0
        for byte in data.dropFirst() {  // Skip '$'
            checksum ^= byte
        }
        return "*\(String(format: "%02X", checksum))"
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
        do {
            try self.pipeline.tick(location.toNMEA())
        } catch {
            os_log(.error, "Pipeline error: %@", error.localizedDescription)
            // Handle error: restart pipeline, notify user, etc.
        }
    }
}
```

**Option B: Async/Await**
```swift
actor PipelineActor {
    var pipeline: BusArrivalPipeline

    func tick(_ nmea: String) async throws {
        try await pipeline.process(nmea)
    }
    
    func handleRecovery() async {
        // Reinitialize pipeline on error
        pipeline = BusArrivalPipeline()
    }
}
```

### Memory Management

**Memory Budget (device-dependent):**
- Route data: ~12 KB (load from JSON, convert to Swift structs)
- LUTs: ~1 KB (precompute as `[UInt8]` arrays)
- Runtime state: < 1 KB (single struct instance)
- **Total: ~14 KB** (well within iOS limits, even on older devices)

**Memory Safety:**
```swift
// Use value types (structs) for automatic memory management
struct RouteData {
    let nodes: [RouteNode]
    let stops: [Stop]
    let grid: SpatialGrid
}

// Avoid retain cycles with closures
class LocationManager {
    var pipeline: BusArrivalPipeline?
    
    func setupCallback() {
        // [weak self] prevents retain cycle
        locationManager.delegate = self
    }
}
```

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
#define GPS_QUEUE_SIZE 32
#define GPS_TASK_STACK 4096

static QueueHandle_t gps_queue;

void gps_task(void* arg) {
    char nmea_buffer[256];
    
    while (1) {
        int len = uart_read_bytes(GPS_UART_NUM, nmea_buffer, sizeof(nmea_buffer), 100 / portTICK_PERIOD_MS);
        if (len > 0 && validate_nmea(nmea_buffer, len)) {
            NMEAMessage msg = {0};
            strncpy(msg.data, nmea_buffer, sizeof(msg.data) - 1);
            msg.len = len;
            
            // Send to pipeline task (non-blocking)
            if (xQueueSend(gps_queue, &msg, 0) != pdTRUE) {
                ESP_LOGW("GPS", "Queue full, dropping NMEA message");
            }
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
    
    // Validate header magic number
    if (header->magic != ROUTE_MAGIC) {
        ESP_LOGE("ROUTE", "Invalid route data (magic mismatch)");
        return NULL;
    }
    
    *count = header->node_count;
    ESP_LOGI("ROUTE", "Loaded %zu nodes", *count);
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
    // Create queue for GPS → Pipeline communication
    gps_queue = xQueueCreate(GPS_QUEUE_SIZE, sizeof(NMEAMessage));
    if (gps_queue == NULL) {
        ESP_LOGE("APP", "Failed to create GPS queue");
        return;
    }

    // Create GPS task
    xTaskCreate(gps_task, "gps", GPS_TASK_STACK, NULL, 5, NULL);

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

**Note:** `PipelineState` is defined in `01-data-formats.md` — refer to that document for the complete structure.

## Platform-Specific Testing

### iOS Testing

**Unit Tests:**
```swift
import XCTest

class KalmanFilterTests: XCTestCase {
    func testKalmanConvergence() {
        var kf = KalmanState()
        
        // Feed constant velocity
        for i in 0..<100 {
            kf.update(z_cm: i * 100, v_gps: 100)
        }
        
        // Should converge to ~100 cm/s
        XCTAssertEqual(kf.v_cms, 100, accuracy: 5)
    }
    
    func testNMEAEmulation() {
        let loc = CLLocation(coordinate: CLLocationCoordinate2D(latitude: 37.7749, longitude: -122.4194),
                            altitude: 0, horizontalAccuracy: 5, verticalAccuracy: 0,
                            course: 90.0, speed: 10.0, timestamp: Date())
        
        let nmea = loc.toNMEA()
        XCTAssertTrue(nmea.hasPrefix("$GPRMC"))
        XCTAssertTrue(nmea.contains("*"))  // Has checksum
    }
}
```

**Integration Tests:**
- Test CoreLocation callback flow
- Verify background execution permission
- Test memory allocation under load

### ESP32 Testing

**Unit Tests (host-based):**
```c
void test_kalman_prediction() {
    KalmanState kf;
    kalman_init(&kf);
    
    // Test prediction step
    int32_t s_pred = kf.s_cm + kf.v_cms;
    TEST_ASSERT_EQUAL_INT32(s_pred, kalman_predict(&kf));
}

void test_gaussian_lut() {
    // Test LUT bounds checking
    TEST_ASSERT_EQUAL_UINT8(255, gaussian_lut(0, 1));      // Peak
    TEST_ASSERT_EQUAL_UINT8(0, gaussian_lut(1000000, 1)); // Overflow
}
```

**Integration Tests (on-device):**
```c
void test_pipeline_tick() {
    PipelineState pipeline;
    pipeline_init(&pipeline);
    
    const char* test_nmea = "$GPRMC,123456.00,A,3744.1234,N,12225.1234,W,10.0,90.0,,,,,D*42";
    
    esp_err_t err = pipeline_tick(&pipeline, test_nmea);
    TEST_ASSERT_ESP_OK(err);
    
    // Verify state updated
    TEST_ASSERT_NOT_EQUAL(0, pipeline.kalman.s_cm);
}
```

## Performance Considerations

### Timing Constraints

**1 Hz Pipeline Tick:**
- **Budget:** 1000 ms per tick
- **Typical usage:** 1-5 ms per tick
- **Headroom:** > 995 ms available for other tasks

**GPS Processing:**
- NMEA parsing: < 1 ms
- Kalman update: < 0.5 ms
- Map matching: < 2 ms
- Bayesian detection: < 1 ms

### iOS Performance

**Best Practices:**
```swift
// Use background queue for heavy computation
DispatchQueue.global(qos: .userInitiated).async {
    let result = self.pipeline.process(nmea)
    
    // Update UI on main queue
    DispatchQueue.main.async {
        self.updateUI(result)
    }
}

// Profile with Instruments
// - Time Profiler: Check CPU usage
// - Allocations: Verify memory budget
// - Leaks: Detect retain cycles
```

**Common Pitfalls:**
- **Main thread blocking:** Never run pipeline on UI thread
- **Memory leaks:** Use `[weak self]` in closures
- **Battery drain:** Minimize location update frequency

### ESP32 Performance

**CPU Monitoring:**
```c
void monitor_cpu_usage() {
    static uint32_t idle_ticks = 0;
    static uint32_t total_ticks = 0;
    
    uint32_t current_idle = xTaskGetIdleRunTimeCounter();
    uint32_t current_total = xTaskGetTickCount();
    
    uint32_t idle_delta = current_idle - idle_ticks;
    uint32_t total_delta = current_total - total_ticks;
    
    uint8_t cpu_usage = 100 - (idle_delta * 100 / total_delta);
    ESP_LOGI("PERF", "CPU: %d%%", cpu_usage);
    
    idle_ticks = current_idle;
    total_ticks = current_total;
}
```

**Memory Monitoring:**
```c
void check_memory() {
    ESP_LOGI("MEM", "Free heap: %d bytes", esp_get_free_heap_size());
    ESP_LOGI("MEM", "Min free: %d bytes", esp_get_minimum_free_heap_size());
    
    // Alert if memory is low
    if (esp_get_minimum_free_heap_size() < 10240) {
        ESP_LOGW("MEM", "Low memory condition detected!");
    }
}
```

**Timing Verification:**
```c
void profile_pipeline_tick() {
    uint32_t start = esp_timer_get_time();
    
    pipeline_tick(&pipeline, &nmea_msg);
    
    uint32_t elapsed = esp_timer_get_time() - start;
    ESP_LOGI("PERF", "Pipeline tick: %lu us", elapsed);
    
    // Should be < 5000 us (5 ms)
    if (elapsed > 5000) {
        ESP_LOGW("PERF", "Pipeline tick exceeded budget!");
    }
}
```

### Optimization Checklist

- [ ] Profile before optimizing (measure actual bottlenecks)
- [ ] Use integer math on embedded platforms
- [ ] Precompute LUTs at startup (not runtime)
- [ ] Minimize memory allocations in hot paths
- [ ] Use queue-based messaging (avoid polling)
- [ ] Set appropriate task priorities
- [ ] Monitor CPU usage under load
- [ ] Verify no memory leaks (long-running tests)
