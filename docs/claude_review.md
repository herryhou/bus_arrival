現在我已讀完整個 codebase，給出完整 review。

---

## Code Review：pico2-firmware + pipeline

整體架構清晰，分層設計（shared / gps_processor / detection / pico2-firmware）合理，spec 與程式碼對照良好。但發現數個嚴重問題，分級列出。

---

### 🔴 S1 — 阻斷功能的 Critical Bug

#### 1. `uart.rs`：UART 每秒只讀 1 byte，GPS 句子永遠收不到

```rust
// main.rs：每次 loop 只呼叫一次 read_nmea_sentence，然後 sleep 1 秒
match uart::read_nmea_sentence(&mut uart, &mut line_buf) { ... }
Timer::after(Duration::from_secs(1)).await;
```

`read_nmea_sentence` 每次呼叫只做一次 `blocking_read(&mut byte)` — 讀 1 個 byte。NMEA 句子約 70–82 bytes，1 Hz 速率下需要 70–82 秒才能湊齊一句。GPS 模組 9600 baud 會在 ~83 ms 內送完整句，但 firmware 永遠跟不上。

**修法**：`read_nmea_sentence` 內部要 loop 讀到 `\r\n` 為止，或用 async UART 接收。

---

#### 2. `main.rs`：`.route_data` section 永遠是零，startup 必然 panic

```rust
#[link_section = ".route_data"]
static ROUTE_DATA: [u8; 64 * 1024] = [0u8; 64 * 1024];  // BSS，全零

// ...
let route_data = shared::binfile::RouteData::load(&ROUTE_DATA)
    .expect("Failed to load route data");  // 必然 panic
```

`build.rs` 把 `route_data.bin` 複製到 `OUT_DIR` 並加入 link search path，但沒有 linker script 把該 binary 塞入 `.route_data` section。Static 停留在 BSS（全零），`load()` 的 magic/CRC 驗證會失敗。

注意：`build.rs` 的 `cargo:rustc-link-search` 對 section data embedding 無效，需要 `include_bytes!` 或自訂 linker script（`AT(LOADADDR(...))` 模式）。

---

#### 3. `lut.rs`：`build_gaussian_lut()` const fn 實作幾乎全錯

設計意圖：index i → x = i/64.0 → exp(−x²/2)。

但 const fn 把 `x = i as i32`（未除以 64），`x2 = x*x = i²`（實際是 (i/64)² 的 4096 倍）。加上整數除法的截斷，結果變成只有 4 個台階的階梯函數：

| index i | LUT 值 | 正確 exp(−(i/64)²/2) |
|---------|--------|----------------------|
| 0–7 | 255 | 255–253 ✓ |
| 8–15 | 200 | 244–224 ✗ |
| 16–23 | 100 | 211–184 ✗ |
| 24–29 | 40 | 175–162 ✗ |
| **≥ 30** | **0** | **159 → ~154 @ i=64** ✗ |

`idx1 = (d_cm * 64 / 2750)`，在 d_cm = 2750 cm（一個 sigma）時 idx1 = 64，**GAUSSIAN_LUT[64] = 0**。正確值應為 154。超過 ~1289 cm（13m）後 p1 就全是 0。到站偵測在任何實際情境下都無法觸發。

---

#### 4. `lut.rs`：`build_logistic_lut()` 邏輯錯誤，v_stop 點偏移

正確：L(v_stop=200) = 1/(1+exp(0)) = 0.5 → 127。

Const fn 在 delta=0 時：`exp_val = 2 + (0/100) = 2`，`L = 255/(1+2) = 85` → 33%。誤差 17 個百分點。v=0 時 (`delta=-200`)：`exp_val=0`，`L=255/(1+0)=255`。真實值是 `1/(1+exp(-2))=0.88` → 224。

Logistic LUT 和 host pipeline 的 `build_logistic_lut()` (用 f64 計算) 輸出根本不同，threshold THETA_ARRIVAL=191 在 firmware 中意義已失效。

---

### 🟠 S2 — 重大行為偏差

#### 5. `uart.rs`：`read_nmea_sentence` 的 lifetime 標記 unsound

```rust
pub fn read_nmea_sentence<'a>(
    uart: &mut Uart<'a, embassy_rp::uart::Blocking>,
    line_buf: &mut UartLineBuffer,
) -> Result<Option<&'a str>, ()> {
    // ...
    let sentence = unsafe {
        let slice = core::slice::from_raw_parts(line_buf.buffer.as_ptr(), line_buf.len - 2);
        core::str::from_utf8_unchecked(slice)
    };
    return Ok(Some(sentence));
```

回傳的 `&'a str` 的 lifetime `'a` 綁的是 `uart`，但實際指向 `line_buf.buffer`。Caller 只要在使用 sentence 之後呼叫 `line_buf.reset()`（main.rs 確實這樣做），就會有 dangling reference（目前碰巧安全，但 borrow checker 無法保護）。

應該把 lifetime 綁到 `line_buf`：`&'buf UartLineBuffer` → 回傳 `Option<&'buf str>`，取消 unsafe block。

---

#### 6. `detection.rs` (firmware)：未實作 adaptive weights，close stop 問題未修

`compute_arrival_probability()` 使用固定權重 (13, 6, 10, 3)，但 pipeline 有 `arrival_probability_adaptive()` 在 next stop < 120m 時降低 p4 權重。Firmware 沒有同步，v8.x 針對近距離站點的修正在 pico 上無效。

---

#### 7. `nmea.rs`：未處理 `$GNRMC`、`$GNGGA` 等 GNSS 多星座句

```rust
match parts_slice.first() {
    Some(&"$GPRMC") => ...,
    Some(&"$GNGSA") => ...,
    Some(&"$GPGGA") => ...,
    _ => None,  // $GNRMC, $GNGGA, $GNRMC 全部丟棄
}
```

u-blox 等常見模組在多星座模式下輸出 `$GNRMC` 和 `$GNGGA` 而非 `$GPRMC`/`$GPGGA`，整個 NMEA parser 會靜默無輸出。

---

#### 8. `kalman.rs`：`handle_outage` 回傳 `state.v_cms` 而非 `dr.filtered_v`

```rust
ProcessResult::DrOutage {
    s_cm: state.s_cm,   // 已用 DR 更新 ✓
    v_cms: state.v_cms, // ← 應為 dr.filtered_v（有 decay）
}
```

DR 期間速度衰減（`dr.filtered_v = dr.filtered_v * 9 / 10`），但回傳的 v_cms 是舊的 Kalman 濾波值，不反映衰減。downstream 的 `compute_arrival_probability` 拿到的 speed 偏高，`p2` 偏低。

---

### 🟡 S3 — Edge Case / 可靠性

#### 9. `state.rs` (firmware)：stop_count > 256 時靜默丟棄

```rust
for i in 0..stop_count {
    let _ = stop_states.push(StopState::new(i as u8));  // 超過 256 靜默失敗
}
```

超過 256 站的路線（大型環形路線）會在初始化時靜默截斷，後續不會有任何錯誤，但第 257 站後的站點永遠偵測不到。至少應加 `defmt::warn!`。

---

#### 10. `state.rs` (firmware)：同一 tick 最多回傳一個 arrival，多站同時觸發時漏報

```rust
for stop_idx in active_indices {
    // ...
    match event {
        StopEvent::Arrived => {
            return Some(ArrivalEvent { ... });  // 立即 return，跳過後續站點
        }
    }
}
```

近距離站點走廊重疊時，若兩站同時觸發 Arrived，第二個永遠不會回傳。可考慮 return `heapless::Vec<ArrivalEvent, 2>` 或在 loop 後處理。

---

### 🔵 S4 — 程式碼品質

**11. `find_active_stops` 重複定義三處**：`detection/corridor.rs`、`output.rs`（gps_processor）、`pico2-firmware/detection.rs`。應統一在 `shared` 或 `detection::corridor`，其他 re-export。

**12. `StopState::reset()` 是 documented NO-OP**，但函式依然存在且被測試。若 v8.6 one-time announcement rule 是永久決策，應移除 `reset()` 並更新所有 call sites，避免未來維護者誤以為它有效。

**13. `parse_rmc` 的 heading 轉換邏輯可簡化**：

```rust
let heading_cdeg = f64_round(heading_deg * 100.0) as i32;
let heading_cdeg = if heading_cdeg > 18000 { heading_cdeg - 36000 } else { heading_cdeg };
```

這個 shadowing 可讀性差，建議用 `let normalized = ...` 分開。

**14. `ROUTE_DATA` 大小寫在規格書和程式碼之間不一致**：doc comment 說 "~10-12 KB" 但 static 宣告 64 KB，浪費 Flash。

---

### 修復優先順序建議

| 優先 | 項目 | 影響 |
|------|------|------|
| 立即 | UART 讀取 loop (#1) | firmware 完全無法運作 |
| 立即 | Route data embedding (#2) | firmware startup panic |
| 立即 | Gaussian LUT (#3) + Logistic LUT (#4) | 到站偵測完全失效 |
| 短期 | Lifetime unsoundness (#5) | UB 風險 |
| 短期 | `$GNRMC`/`$GNGGA` (#7) | 部分 GPS 模組無法工作 |
| 短期 | Adaptive weights (#6) | 近距離站點誤報 |
| 中期 | DR v_cms (#8)、stop overflow (#9)、multi-arrival (#10) | 邊界情境 |

======
## 2nd Code Review

### 🔴 N1 — Main Loop 仍然每 Sentence 睡 1 秒（GPS burst 丟失）

`read_nmea_sentence` 現在每次呼叫讀到 `\r\n` 回傳一句，然後主迴圈就 sleep 1 秒：

```rust
loop {
    match uart::read_nmea_sentence(&mut uart, &mut line_buf) {
        Ok(Some(sentence)) => {
            state.nmea.parse_sentence(sentence);  // RMC → None，無 GpsPoint
            line_buf.reset();
        }
        ...
    }
    Timer::after(Duration::from_secs(1)).await;  // ← 睡 1 秒
}
```

GPS 模組每秒以 burst 方式送出多句（RMC → GSA → GGA，約在 200ms 內完成）。主迴圈讀完 RMC 後睡 1 秒，這期間 GSA 抵達並填滿 RP2350 UART RX FIFO（僅 32 bytes < 80 bytes/句），**溢出丟失**。下一秒讀到的是下一個 epoch 的 GSA，與這秒的 RMC 永遠不會配對 → `NmeaState` 無法產生完整 `GpsPoint`。

**正確做法**：在 sleep 之前把當前 burst 全部排空：

```rust
loop {
    // 一次排空所有 burst sentences
    loop {
        match uart::read_nmea_sentence(&mut uart, &mut line_buf) {
            Ok(Some(sentence)) => {
                if let Some(gps) = state.nmea.parse_sentence(sentence) {
                    // ... process ...
                }
                line_buf.reset();
            }
            Ok(None) => break,   // FIFO 暫時為空
            Err(_) => { line_buf.reset(); break; }
        }
    }
    Timer::after(Duration::from_secs(1)).await;
}
```

這要求 `blocking_read` 在 FIFO 空時返回 `Err`（embassy-rp 的 blocking UART 預設行為），否則需要帶 timeout 的 async UART。若 GPS 模組設定輸出 GGA-only（GGA 一句即可完成 GpsPoint），目前的設計雖然可以運作，但應在程式碼中明確說明此假設。

---

### 🟠 N2 — `#[link_section]` 標在 `&[u8]` 上只會放 fat pointer

```rust
#[link_section = ".route_data"]
static ROUTE_DATA: &[u8] = include_bytes!("../../../test_data/ty225_normal.bin");
```

`static ROUTE_DATA: &[u8]` 是一個 **fat pointer**（32-bit 平台上 8 bytes：4B 位址 + 4B 長度）。`#[link_section]` 只把這 8 bytes 放進 `.route_data`，實際的二進位資料由 `include_bytes!` 放進 `.rodata`。功能上正確（指標有效，資料在 Flash 中），但 `link_section` 沒有起任何作用，也容易讓維護者誤解。

另外，`load(&ROUTE_DATA)` 中的 `&` 是多餘的（`ROUTE_DATA` 本身已是 `&[u8]`，傳入 `&&[u8]` 後靠 auto-deref 展開）。改為 `load(ROUTE_DATA)` 更清楚。

建議：

```rust
// 移除 link_section（無效），直接 include_bytes
static ROUTE_DATA: &[u8] = include_bytes!("../../../test_data/ty225_normal.bin");

// load 時不用加 &
let route_data = shared::binfile::RouteData::load(ROUTE_DATA)
    .expect("Failed to load route data");
```

---

### 🟡 N3 — 新增 test 與既有 test 對同一輸入期望值矛盾

新增的 `parse_gnrmc_valid` 測試：
```rust
assert_eq!(state.point.lon_cdeg, 12128); // 121.286496...° * 100 (truncated)
```

既有的 `parse_rmc_valid` 測試（相同輸入 `"12117.1898"`）：
```rust
assert_eq!(state.point.lon_cdeg, 12129); // 121.29° * 100 (rounded)
```

計算驗算：`parse_lon("12117.1898", "E")` → `121.0 + 17.1898/60 = 121.28649...` → `× 100 = 12128.64...` → `as i32 = 12128`（截斷，非四捨五入）。

12129 是錯的。`parse_rmc_valid` 的期望值 `12129` 是既有 bug，一直潛伏，新增 GNRMC 測試的 `12128` 才是正確值。應把 `parse_rmc_valid` 的 `lon_cdeg` 期望值改成 `12128`。

---

### 🔵 N4 — Logistic LUT doc comment 符號錯誤

```rust
/// - i=0 (v=0): exp(2)≈7.39 → 1/8.39≈0.119 → 30 (actual: 224 due to different formula)
```

`k*(v - v_stop) = 0.01*(0 - 200) = -2`，應是 `exp(-2) ≈ 0.135`，`1/(1+0.135) ≈ 0.881 → 224`。Comment 把指數符號寫反了，且 "due to different formula" 是錯的——只有一個公式，值是 224，不是 30。直接改為正確說明即可。

---

### 未修正的舊問題（供追蹤）

- **#10**（多站同 tick 同時到站漏報）：仍是 `return Some(...)` 在迴圈中 early return，只回報第一個。
- **#11**（`find_active_stops` 三份重複）：未處理。
- **#12**（`reset()` NO-OP）：未移除。

===

## 3rd Code Review 0 Semantic Type 擴充建議（彙整版）

### 新增型別定義（`shared/src/lib.rs`）

| 型別名稱 | 原始型別 | 單位 | 取代目標 |
|---------|---------|------|---------|
| `GeoCdeg` | `i16` | 0.01° | `lat_cdeg`, `lon_cdeg`（現誤用 `HeadCdeg`） |
| `StopIdx` | `u8` | — | 所有 `stop_idx: u8`, `last_index: u8` |
| `DistMm` | `i32` | 1 mm | `seg_len_mm` |
| `TimeS` | `u64` | 1 sec | `timestamp`, `time`（各 struct 命名不統一） |
| `DwellS` | `u16` | 1 sec | `dwell_time_s` |
| `HdopX10` | `u16` | 0.1 | `hdop_x10` |

---

### 優先順序

**立即修正（語意錯誤）**

`lat_cdeg`/`lon_cdeg` 目前型別是 `HeadCdeg`（意為「方向角」），但緯經度不是方向角，造成語意混淆，且編譯器無法攔截將兩者傳錯的 bug：

```rust
// 改前：三個欄位型別相同，無法區分
pub lat_cdeg:     HeadCdeg,
pub lon_cdeg:     HeadCdeg,
pub heading_cdeg: HeadCdeg,

// 改後：編譯器可區分
pub lat_cdeg:     GeoCdeg,
pub lon_cdeg:     GeoCdeg,
pub heading_cdeg: HeadCdeg,
```

**短期改善（一致性）**

`StopIdx` — 下列五處都是「站點索引」卻各自用裸 `u8`，加上型別後 grep 可一次找齊所有使用點：

```rust
StopState.index, StopState.last_announced_stop,
ArrivalEvent.stop_idx, DepartureEvent.stop_idx,
find_stop_index(last_index: u8)
```

**其餘**（`DistMm`, `TimeS`, `DwellS`, `HdopX10`）為補全現有命名體系的空缺，一併修即可。

---

### 不建議採用：`Weight8 = u8`

Gemini 建議將機率融合權重（`w1`~`w4`）定義為 `Weight8 = u8`。但權重在運算中立即被提升為 `u32` 做乘法，加型別反而需要額外 `as u32` 轉型，得不償失。魔法數字問題用具名常數解決即可：

```rust
const WEIGHTS_NORMAL: (u32, u32, u32, u32) = (13, 6, 10, 3);
const WEIGHTS_CLOSE:  (u32, u32, u32, u32) = (14, 7, 11, 0);
```

---

### 同場加映：裸 `i32` 應改用既有 `DistCm`

以下欄位語意明確是「cm 距離」，卻沒用現有的 `DistCm`：

```rust
// RouteData
pub x0_cm: i32  →  pub x0_cm: DistCm
pub y0_cm: i32  →  pub y0_cm: DistCm

// SpatialGridView
pub grid_size_cm: i32  →  pub grid_size_cm: DistCm
```