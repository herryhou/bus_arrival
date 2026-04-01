# Pico 2 Pipeline Design Spec

## 目標

將現有的 bus arrival detection pipeline 移植到 Pico 2 W，實現：
- 程式碼重用：桌面版與嵌入式版共享核心邏輯 (Single Source of Truth)
- 記憶體限制：< 5KB RAM 使用
- Route data：從外部 SPI Flash XIP 載入
- 輸入：UART GPS 模組
- 輸出：JSON 格式事件 (UART)

## 架構

### Crate 結構

```
crates/
├── shared/              # 現有，加上 feature gating
│   ├── Cargo.toml       # [features] std = ["serde", "crc32fast/std"]
│   ├── lib.rs           # 類型定義 (no_std 相容)
│   └── binfile.rs       # RouteData XIP 載入 (zero-copy)
│
├── pipeline/            # 現有，加上 feature gating
│   ├── Cargo.toml       # [features] std = ["dep:serde_json"]
│   ├── lib.rs           # 核心邏輯 (no_std 相容)
│   ├── gps_processor/   # NMEA + Kalman + Map matching
│   └── detection/       # FSM + Probability
│
└── pico2-firmware/      # 新增：Pico 2 W 固件
    ├── Cargo.toml       # shared = { path = "../shared", default-features = false }
    ├── memory.x         # Linker script for XIP
    └── src/
        ├── main.rs      # UART GPS → JSON output
        └── uart.rs      # UART driver
```

### 依賴關係

```
┌─────────────────┐     ┌─────────────────┐
│  pipeline       │     │ pico2-firmware  │
│  (std feature)  │     │ (no_std)        │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────────┬───────────┘
                     │
         ┌───────────▼───────────┐
         │       shared          │
         │   (no_std + std)      │
         │   default-features: std│
         └───────────────────────┘
```

### Feature Gating 設定

```toml
# shared/Cargo.toml
[features]
default = ["std"]
std = ["serde", "crc32fast/std"]

[dependencies]
serde = { workspace = true, optional = true }
crc32fast = { workspace = true }
```

```toml
# pipeline/Cargo.toml
[features]
default = ["std"]
std = ["shared/std", "dep:serde_json"]

[dependencies]
shared = { path = "../shared", default-features = false }
serde_json = { workspace = true, optional = true }
```

```toml
# pico2-firmware/Cargo.toml
[dependencies]
shared = { path = "../shared", default-features = false }
# 使用 serde_json_core 取代 serde_json
serde_json_core = "0.6"
```

## 記憶體估算

### Runtime SRAM

| 組件 | 大小 | 說明 |
|------|------|------|
| NmeaState | 64 bytes | 一個 GpsPoint |
| KalmanState | 24 bytes | s_cm, v_cms, last_seg_idx |
| DrState | 24 bytes | last_gps_time, last_valid_s, filtered_v |
| StopState × 256 | ~1.8KB | 每個 stop ~7 bytes |
| UART buffers | ~512 bytes | 輸入/輸出緩衝 |
| **Total** | **~2.5KB** | 遠低於 5KB 限制 |

### StopState 詳細大小

```rust
pub struct StopState {
    index: u8,              // 1 byte
    fsm_state: FsmState,    // 1 byte (enum)
    dwell_time_s: u16,      // 2 bytes
    last_probability: u8,   // 1 byte
    last_announced_stop: u8,// 1 byte
    announced: bool,        // 1 byte
}  // 總共 7 bytes (可能 alignment 到 8 bytes)
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
    ├── SpatialGrid (sparse, v8.8)
    └── LUTs (gaussian, logistic)
```

### 載入方式

```rust
#[link_section = ".route_data"]
static ROUTE_DATA: [u8; 128*1024] = [0u8; 128*1024];

let route_data = shared::binfile::RouteData::load(&ROUTE_DATA)?;

// 直接使用 route_data，不複製到 RAM
// RouteData 內部使用指標指向 Flash 資料 (zero-copy)
for (idx, stop) in route_data.stops().iter().enumerate() {
    // stop 是從 Flash 讀取的副本
}
```

## JSON 輸出格式

### 桌面版 (std)

```rust
#[cfg(feature = "std")]
fn emit_event_uart(event: &Event) -> Result<(), Error> {
    let json = serde_json::to_string(event)?;
    println!("{}", json);
    Ok(())
}
```

### 嵌入式版 (no_std)

```rust
#[cfg(not(feature = "std"))]
fn emit_event_uart<UART: Write<u8>>(
    uart: &mut UART,
    event: &Event,
) -> Result<(), Error> {
    let mut buf = [0u8; 128];
    let len = serde_json_core::to_string(&buf, &event)?;
    for &b in &buf[..len] {
        nb::block!(uart.write(b))?;
    }
    nb::block!(uart.write(b'\n'))?;
    Ok(())
}
```

### 事件格式

```json
// Arrival
{"type":"arrival","time":1234567890,"stop_idx":5,"s_cm":15000,"v_cms":100,"probability":200}

// Departure
{"type":"departure","time":1234567895,"stop_idx":5,"s_cm":16000,"v_cms":500}
```

## 條件編譯處理

### serde_json vs serde_json_core

```rust
// pipeline/lib.rs

#[cfg(feature = "std")]
use serde_json;

#[cfg(not(feature = "std"))]
use serde_json_core as serde_json;

// 統一的介面
pub fn to_string<T: serde::Serialize>(
    buf: &mut [u8],
    value: &T,
) -> Result<usize, Error> {
    #[cfg(feature = "std")]
    {
        let s = serde_json::to_string(value)?;
        buf[..s.len()].copy_from_slice(s.as_bytes());
        Ok(s.len())
    }
    #[cfg(not(feature = "std"))]
    {
        serde_json_core::to_string(buf, value).map_err(Into::into)
    }
}
```

## 測試

### 策略

1. 桌面版產生 ground truth (使用 std feature)
2. 嵌入式版處理相同資料 (no_std feature)
3. 比對輸出一致性

### 整合測試

```rust
#[test]
fn test_pipeline_matches_host() {
    // 載入 route_data.bin
    let route_bytes = std::fs::read("test_data/route_data.bin").unwrap();
    let route_data = shared::binfile::RouteData::load(&route_bytes).unwrap();

    // 載入測試 NMEA
    let nmea_data = std::fs::read_to_string("test_data/test.nmea").unwrap();

    // 執行 pipeline (使用 std feature)
    let results = run_pipeline(&route_data, &nmea_data);

    // 載入 ground truth
    let expected: Vec<Event> = ...;

    // 比對
    assert_eq!(results, expected);
}
```

### 嵌入式測試

```bash
# 在主機上執行 no_std 測試
cargo test --package pipeline --no-default-features

# 交叉編譯測試
cargo test --package pipeline --target thumbv6m-none-eabi --no-default-features
```

## 實作順序

1. **shared feature gating** - 加入 `std` feature
2. **pipeline feature gating** - 移除 std 依賴，使用條件編譯
3. **serde_json 統一介面** - 抽象序列化差異
4. **pico2-firmware** - 建立 Pico 2 W 專案
5. **UART driver** - 實作 GPS 輸入和 JSON 輸出
6. **測試** - 驗證 std 和 no_std 輸出一致性
