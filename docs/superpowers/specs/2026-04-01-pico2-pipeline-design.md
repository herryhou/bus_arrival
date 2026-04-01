# Pico 2 Pipeline Design Spec

## 目標

將現有的 bus arrival detection pipeline 移植到 Pico 2 W，實現：
- 程式碼重用：桌面版與嵌入式版共享核心邏輯
- 記憶體限制：< 50KB RAM 使用
- Route data：從外部 SPI Flash XIP 載入
- 輸入：UART GPS 模組
- 輸出：JSON 格式事件 (UART)

## 架構

### Crate 結構

```
crates/
├── shared/              # 現有 (已經 no_std 相容)
│   ├── lib.rs           # 類型定義
│   └── binfile.rs       # RouteData XIP 載入
│
├── embedded-core/       # 新增：no_std 核心邏輯
│   ├── lib.rs           # 模組導出
│   ├── nmea.rs          # NMEA parser (heapless)
│   ├── kalman.rs        # Kalman filter + GPS processing
│   ├── map_match.rs     # Map matching
│   ├── state_machine.rs # StopState FSM
│   ├── probability.rs   # Probability model (static LUTs)
│   └── io.rs            # Trait 定義
│
├── pipeline/            # 現有 (std)，重構為使用 embedded-core
│   ├── lib.rs           # 桌面版 wrapper
│   └── main.rs          # CLI 工具
│
└── pico2-firmware/      # 新增：Pico 2 W 固件
    ├── Cargo.toml       # rp2040-hal, embedded-hal
    ├── memory.x         # Linker script for XIP
    └── src/
        ├── main.rs      # UART GPS → JSON output
        └── uart.rs      # UART driver
```

### 依賴關係

```
┌─────────────────┐     ┌─────────────────┐
│  pipeline       │     │ pico2-firmware  │
│  (std binary)   │     │ (no_std binary) │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────────┬───────────┘
                     │
         ┌───────────▼───────────┐
         │   embedded-core       │
         │   (no_std library)    │
         └───────────┬───────────┘
                     │
         ┌───────────▼───────────┐
         │       shared          │
         │   (no_std types)      │
         └───────────────────────┘
```

## embedded-core Crate

### 設計原則

1. **no_std**：不依賴 `std`，只使用 `core` 和 `alloc`
2. **heapless 優先**：避免動態記憶體分配，使用固定大小陣列
3. **靜態 LUTs**：Probability model 使用查表法

### 核心類型

```rust
pub struct EmbeddedPipeline<'a> {
    route_data: &'a shared::binfile::RouteData<'a>,
    nmea_state: nmea::NmeaState,
    kalman_state: KalmanState,
    dr_state: DrState,
    stop_states: heapless::Vec<state_machine::StopState, 256>,
}
```

### 記憶體估算

| 組件 | 大小 |
|------|------|
| NmeaState | ~64 bytes |
| KalmanState | 24 bytes |
| DrState | ~24 bytes |
| StopState × 256 | ~13KB |
| **Total** | **~13KB** |

## I/O 抽象層

### Trait 定義

```rust
pub trait GpsInput {
    fn read_line(&mut self, buf: &mut [u8]) -> Result<(usize, bool), InputError>;
}

pub trait EventOutput {
    fn emit_arrival(&mut self, event: &ArrivalEvent) -> Result<(), OutputError>;
    fn emit_departure(&mut self, event: &DepartureEvent) -> Result<(), OutputError>;
}
```

### Pico 2 W 實作

```rust
pub struct UartGpsInput<UART> { uart: UART, buffer: [u8; 256] }
pub struct UartEventOutput<UART> { uart: UART }
```

## Route Data XIP

### Flash 配置

```
External SPI Flash (XIP):
├── Firmware code
└── route_data.bin (~30-50KB)
    ├── RouteData header
    ├── RouteNode array
    ├── Stop array
    ├── SpatialGrid (sparse)
    └── LUTs (gaussian, logistic)
```

### 載入方式

```rust
#[link_section = ".route_data"]
static ROUTE_DATA: [u8; 128*1024] = [0u8; 128*1024];

let route_data = shared::binfile::RouteData::load(&ROUTE_DATA)?;
let mut pipeline = EmbeddedPipeline::new(&route_data);
```

## JSON 輸出格式

### Arrival Event

```json
{"type":"arrival","time":1234567890,"stop_idx":5,"s_cm":15000,"v_cms":100,"probability":200}
```

### Departure Event

```json
{"type":"departure","time":1234567895,"stop_idx":5,"s_cm":16000,"v_cms":500}
```

### 序列化

使用 `serde_json_core` (no_std)：
```rust
let mut buf = [0u8; 128];
let len = to_string(&buf, &json_event)?;
// 寫入 UART
```

## 測試

### 策略

1. 桌面版產生 ground truth
2. 嵌入式版處理相同資料
3. 比對輸出

### 整合測試

```rust
#[test]
fn test_pipeline_matches_host() {
    // 載入 route_data.bin 和測試 NMEA
    // 執行 embedded pipeline
    // 比對與 ground truth
}
```

## 實作順序

1. **embedded-core crate** - 從現有程式碼提取核心邏輯
2. **shared binfile** - 確認 XIP 相容性
3. **I/O traits** - 定義抽象層
4. **pico2-firmware** - 實作 UART driver 和 main
5. **pipeline 重構** - 使用 embedded-core
6. **測試** - 驗證輸出一致性
