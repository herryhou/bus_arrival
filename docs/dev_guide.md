# GPS 公車到站偵測系統 — Embedded Rust 專案開發文檔

**硬體平台：** Raspberry Pi Pico 2（RP2350）  
**開發語言：** Embedded Rust（no_std）  
**文件版本：** v1.0  
**依據：** 技術設計報告 v2.1

> **系統指標：** CPU < 8%（@150 MHz，無 FPU）｜SRAM < 1 KB（runtime）｜Flash ~34 KB｜到站準確率 ≥ 97%

---

## 目錄

1. [專案概述](#1-專案概述)
2. [專案目錄結構](#2-專案目錄結構)
3. [語義型別系統](#3-語義型別系統)
4. [核心資料結構](#4-核心資料結構)
5. [模組實作指南](#5-模組實作指南)
6. [Build Script（build.rs）](#6-build-scriptbuildrs)
7. [Flash 資料存取（XIP）](#7-flash-資料存取xip)
8. [並發安全與 Atomic 使用](#8-並發安全與-atomic-使用)
9. [Cargo.toml 設定](#9-cargotoml-設定)
10. [效能預算](#10-效能預算)
11. [測試策略](#11-測試策略)
12. [重要參數速查](#12-重要參數速查)
13. [開發里程碑與 Checklist](#13-開發里程碑與-checklist)

---

## 1. 專案概述

本文檔為 GPS 公車到站偵測系統之 Embedded Rust 專案開發指南，依據技術設計報告 v2.1 整理。系統部署於 Raspberry Pi Pico 2（RP2350，雙核 Cortex-M33，無硬體 FPU），使用 no_std Rust 實作，目標達到 ≥ 97% 到站判定準確率，完整 pipeline CPU 使用率 < 8%。

### 1.1 核心設計原則

- **整數優先：** 以語義化整數型別（cm、0.01°、cm/s）取代浮點，避免 no-FPU 平台軟體浮點的 3–5× 效能懲罰
- **離線預算：** 所有幾何係數於 PC 離線計算後燒錄至 Flash（XIP），runtime 零重算
- **三層防禦：** Stop Corridor + Probability Model + State Machine 三層機制確保到站判定可靠性
- **確定性行為：** 整數運算完全可預測，無浮點累積誤差

### 1.2 硬體約束摘要

| 參數 | 數值 / 說明 |
|------|------------|
| MCU | RP2350（dual-core Cortex-M33，150 MHz） |
| SRAM | 520 KB（runtime 可用 ~400–450 KB） |
| Flash | 2 MB 內建（路線資料預載，XIP） |
| FPU | **無硬體 FPU**（軟體浮點慢 3–5×） |
| GPS 更新率 | 1 Hz（Δt = 1 s） |
| GPS 精度 | ±5–30 m 市區；跳點可達 ±100 m |

### 1.3 浮點 vs 整數效能對比

| 運算類型 | 整數（週期） | 軟體浮點（週期） | 倍率 |
|---------|------------|----------------|------|
| 加法 / 乘法 | 1–2 | 5–15 | 3–8× |
| `sqrtf()` | — | 60–100 | — |
| `expf()` | — | 80–150 | — |
| `cosf()` | — | 50–80 | — |

---

## 2. 專案目錄結構

建議採用 Cargo Workspace 組織，分離離線預處理工具（host std）與嵌入式 firmware（no_std）：

```
bus-arrival/
├── Cargo.toml                  # Workspace root
├── firmware/                   # no_std 嵌入式 crate
│   ├── Cargo.toml
│   ├── build.rs                # LUT 生成 + 路線資料連結
│   ├── memory.x                # RP2350 記憶體佈局
│   ├── src/
│   │   ├── main.rs             # 入口，GPIO / UART 初始化
│   │   ├── types.rs            # 語義整數型別別名
│   │   ├── lut.rs              # Gaussian / Logistic LUT 查表
│   │   ├── route_data.rs       # Flash 靜態資料存取（XIP）
│   │   ├── pipeline/
│   │   │   ├── mod.rs          # GpsPipeline 主結構
│   │   │   ├── map_match.rs    # 模組 ③④：Spatial Index + Map Matching
│   │   │   ├── projection.rs   # 模組 ⑤：Segment Projection
│   │   │   ├── speed_filter.rs # 模組 ⑥：Speed Constraint
│   │   │   ├── kalman.rs       # 模組 ⑦：1D Kalman Filter
│   │   │   └── dead_reckoning.rs # 模組 ⑧：Dead-Reckoning
│   │   └── arrival/
│   │       ├── mod.rs          # ArrivalDetector 主結構
│   │       ├── corridor.rs     # 模組 ⑨：Stop Corridor Filter
│   │       ├── probability.rs  # 模組 ⑩：Stop Probability Model
│   │       ├── state_machine.rs # 模組 ⑪：Stop State Machine
│   │       └── recovery.rs     # 模組 ⑫：Stop Index Recovery
│   └── tests/
│       ├── kalman_test.rs
│       └── pipeline_test.rs
├── preprocessor/               # std host 工具 crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # CLI 入口
│       ├── simplify.rs         # 模組 ①：Douglas-Peucker
│       ├── linearize.rs        # 模組 ②：Route Linearization
│       ├── grid_index.rs       # 模組 ③：Spatial Grid Index
│       └── pack.rs             # 序列化為 route_data.bin
├── shared/                     # firmware + preprocessor 共用型別
│   ├── Cargo.toml
│   └── src/lib.rs              # RouteNode, Stop, GridIndex 定義
└── tools/
    ├── calib/calibrate.js      # 附錄 B：權重離線調校
    ├── gen_nmea/gen_nmea.js    # NMEA GPS log generator
    └── replay/replay.js        # GPS log 回放模擬
```

---

## 3. 語義型別系統

全專案統一以下型別別名，確保單位清晰、杜絕混用。放置於 `firmware/src/types.rs`，並由 `shared/src/lib.rs` re-export。

```rust
// firmware/src/types.rs

/// 距離，單位：公分（cm），範圍 ±21 km（i32 上限 ±21,474 km）
pub type DistCm   = i32;

/// 速度，單位：cm/s（0 = 停止，6000 ≈ 216 km/h）
pub type SpeedCms = i32;

/// 方向角，單位：0.01°，範圍 −180°..+180°（即 −18000..18000）
pub type HeadCdeg = i16;

/// 到站概率，0..255 表示 0.0–1.0（精度 1/256 已足）
pub type Prob8    = u8;

/// 距離平方中間值（cm²），兩個 DistCm 相乘前必須升型至此
pub type Dist2    = i64;
```

| 型別 | 單位 | 值域 | 用途 |
|------|------|------|------|
| `DistCm` / `i32` | cm | ±21 km | `cum_dist`、`stop_progress`、dx/dy |
| `SpeedCms` / `i32` | cm/s | 0–6000 | 速度估計與過濾 |
| `HeadCdeg` / `i16` | 0.01° | −180°–+180° | `heading`、`theta_route`、diff |
| `Prob8` / `u8` | 1/256 | 0–255 | 到站概率、LUT 輸出 |
| `Dist2` / `i64` | cm² | — | 點積、距離平方中間值 |

> **不使用 Q16.16 的原因：** 本專案數值尺度天然適合直接整數表示，Q 格式只增加移位操作負擔，語義反而不清晰。

### 3.1 點積溢位保護

```rust
/// 兩個 i32 向量的點積，升型至 i64 避免溢位
/// 兩個 ±2×10⁶ cm 相乘 → 需 i64（上限 9.2×10¹⁸）
fn dot_i64(ax: i32, ay: i32, bx: i32, by: i32) -> i64 {
    (ax as i64) * (bx as i64) + (ay as i64) * (by as i64)
}
```

---

## 4. 核心資料結構

### 4.1 RouteNode — 路線節點（含預算係數）

v8.7 優化後的結構體佈局，共 **32 bytes**（20% 空間節省）。

```rust
/// v8.7 優化後的欄位佈局（repr(C)，ARM Cortex-M33）：
///   offset  0: seg_len_mm   i64   8 bytes  // Segment length in mm (10× precision)
///   offset  8: x_cm         i32   4 bytes  // X coordinate
///   offset 12: y_cm         i32   4 bytes  // Y coordinate
///   offset 16: cum_dist_cm  i32   4 bytes  // Cumulative distance
///   offset 20: dx_cm        i16   2 bytes  // Segment vector X (max ±100m)
///   offset 22: dy_cm        i16   2 bytes  // Segment vector Y (max ±100m)
///   offset 24: heading_cdeg i16   2 bytes  // Heading in 0.01°
///   offset 26: _pad         i16   2 bytes  // Alignment padding
///   total: 32 bytes
///
/// Key Changes from v8.5:
/// - Removed len2_cm2 (i64) - computed at runtime as (seg_len_mm / 10)^2
/// - Upgraded seg_len_cm (i32) to seg_len_mm (i64) for 10x precision
/// - Reduced dx_cm, dy_cm from i32 to i16 (100m max segment constraint)
#[repr(C)]
pub struct RouteNode {
    // ── i64 fields first（8-byte aligned）─────────────────────────────
    pub seg_len_mm:   i64,   // Segment length in millimeters
    // ── i32 fields（4-byte aligned）───────────────────────────────────
    pub x_cm:         i32,
    pub y_cm:         i32,
    pub cum_dist_cm:  i32,
    // ── i16 fields（2-byte aligned）───────────────────────────────────
    pub dx_cm:        i16,   // Segment vector X (max ±100m fits in i16)
    pub dy_cm:        i16,   // Segment vector Y (max ±100m fits in i16)
    pub heading_cdeg: i16,
    pub _pad:         i16,   // alignment padding
}

// 編譯期驗證 — 欄位重排導致尺寸改變時立即失敗
const _: () = assert!(core::mem::size_of::<RouteNode>() == 32);
```

> **v8.7 優化說明：** 進一步優化結構體從 40 → 32 bytes（20% 空間節省）。移除 `len2_cm2`（runtime 計算），升級 `seg_len_cm` → `seg_len_mm`（10× 精度），縮減 `dx_cm/dy_cm` 為 i16。**600 節點節省 4.8 KB Flash**，同時提升長度解析度。

記憶體佔用：600 節點 × 32 bytes = **19.2 KB**（Flash）

### 4.2 Stop — 站點資料

```rust
#[repr(C)]
pub struct Stop {
    pub progress_cm:       DistCm,  // 站點在路線座標上的位置
    pub corridor_start_cm: DistCm,  // 廊道起點（離線預算）
    pub corridor_end_cm:   DistCm,  // 廊道終點（離線預算）
}
// 50 站點 × 12 bytes = 600 bytes（Flash）
```

### 4.3 KalmanState — 卡爾曼濾波器狀態

```rust
pub struct KalmanState {
    pub s_cm:  DistCm,   // 路線進度估計（cm）
    pub v_cms: SpeedCms, // 速度估計（cm/s）
}
// 8 bytes SRAM

impl KalmanState {
    /// 固定整數增益更新：Ks = 51/256 ≈ 0.20，Kv = 77/256 ≈ 0.30
    pub fn update(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms) {
        let s_pred = self.s_cm + self.v_cms; // dt = 1s
        let v_pred = self.v_cms;
        self.s_cm  = s_pred + (51 * (z_cm       - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms  - v_pred)) / 256;
    }
}
```

### 4.4 StopState — 到站狀態機

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum StopPhase {
    Approaching, // 正在接近站點
    Arriving,    // 進入站點區域
    AtStop,      // 確認到站
    Departed,    // 已離站
}

pub struct StopState {
    pub phase:           StopPhase,
    pub active_stop_idx: u8,  // 當前偵測站點索引
    pub dwell_ticks:     u8,  // 停車計時（秒）
    pub last_arrived:    u8,  // 最後確認到站站序（防重複）
}
// ~50 bytes SRAM
```

---

## 5. 模組實作指南

完整 Pipeline 共 12 個模組，分為「離線預處理」（PC 執行一次）與「Runtime 1 Hz」兩類。

### 模組 ① — Polyline 簡化
**檔案：** `preprocessor/src/simplify.rs`

| 項目 | 說明 |
|------|------|
| 演算法 | Douglas-Peucker（RDP）遞迴，容差 ε = 600–800 cm |
| 急彎保護 | 轉彎角 > 2000 cdeg（20°）時降容差至 200–300 cm |
| 站點保護 | 站點 ±3000 cm 範圍強制保留所有節點 |
| 最大段長 | 簡化後路段 > 2500–3000 cm 則插入補充節點 |
| 效果 | 7,200 節點 → ~640 節點，Flash 節省 87% |

---

### 模組 ② — 路線線性化
**檔案：** `preprocessor/src/linearize.rs`

| 項目 | 說明 |
|------|------|
| 座標系 | 平面近似（非 Haversine），段長 30 m 時誤差 < 0.007 cm，完全可接受 |
| 累積距離 | D[0]=0，D[i] = D[i-1] + sqrt(dx²+dy²)，離線 sqrt，cm 存 i32 |
| 預算係數 | 每節點預算 len2_cm2、line_a/b/c、dx/dy、seg_len、heading_cdeg |
| 站點投影 | progress_cm = D[i] + δ·seg_len，確保嚴格單調遞增 |
| 廊道邊界 | corridor_start = max(D[prev]+δ_sep, progress−L_pre)，離線截斷重疊 |

> **為何不用 Haversine？** 平面近似累積誤差 < 5 cm（12 km 路線），與 GPS 誤差（±500–3000 cm）差四到五個數量級。`sqrt` 僅在離線呼叫，runtime 完全避免。

---

### 模組 ③ — 空間格網索引
**檔案：** `preprocessor/src/grid_index.rs`、`firmware/src/pipeline/map_match.rs`

| 項目 | 說明 |
|------|------|
| 格網大小 | Δg = 10,000 cm（100 m） |
| 原點 | x0_cm, y0_cm = 所有節點最小 x/y（離線計算存 Flash） |
| 索引結構 | 每格存覆蓋路段索引列表，Flash 共 ~1.2 KB |
| 速度自適應窗口 | W = ⌈v_cms / avg_seg_len⌉ + 2，最小值 2 |
| Runtime 流程 | GPS 座標 → grid(gx, gy) → 3×3 鄰域 → 候選路段列表 O(k ≈ 5–15) |

---

### 模組 ④ — 方向約束地圖匹配
**檔案：** `firmware/src/pipeline/map_match.rs`

距離計算（無 `sqrt`）— 採用點積投影法：

```rust
// 計算投影參數 t = dot(G - P[i], segment) / len2
let dx = gps_x - node.x_cm;
let dy = gps_y - node.y_cm;
let t_num = (dx as i64 * node.dx_cm as i64) + (dy as i64 * node.dy_cm as i64);

// 限制投影至路段範圍 [0, len2]
let t = if t_num < 0 { 0 } else if t_num > node.len2_cm2 { node.len2_cm2 } else { t_num };

// 計算投影點
let px = node.x_cm + ((t * node.dx_cm as i64 / node.len2_cm2) as i32);
let py = node.y_cm + ((t * node.dy_cm as i64 / node.len2_cm2) as i32);

// 距離平方
let d2 = ((gps_x - px) as i64).pow(2) + ((gps_y - py) as i64).pow(2);
```

**備註：** `dx_cm`、`dy_cm`、`len2_cm2` 已預算於 RouteNode，runtime 僅需整數運算。

Heading Ramp（漸進加權，非二元切換）：

```rust
/// heading 懲罰權重，0..=256（0=靜止無懲罰，256=全速全效）
/// v_ramp = 83 cm/s（3 km/h）
pub fn heading_weight(v_cms: i32) -> i32 {
    ((v_cms * 256) / 83).min(256)
}

/// 路段評分 = d² + λ × diff² × w_h >> 8
pub fn heading_penalty(
    gps_heading: HeadCdeg,
    seg_heading: HeadCdeg,
    v_cms: i32,
    lambda: i32,
) -> i64 {
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as i64;
    let w    = heading_weight(v_cms) as i64;
    (lambda as i64 * diff * diff * w) >> 8
}
```

| 速度 | 舊設計（二元切換） | 新設計（漸進 Ramp） |
|------|-----------------|-------------------|
| 0 cm/s | heading 無效 | 權重 0（相同） |
| 40 cm/s | heading 無效 | 權重 ≈ 49%（部分約束） |
| 83 cm/s | heading 突然全效 | 權重 100%（平滑過渡） |
| > 83 cm/s | heading 全效 | 全效（相同） |

可選預篩（速度正常時才啟用）：

```rust
if v_cms > 83 && !heading_within(gps_heading, seg.heading_cdeg, 9000) {
    continue; // 90° 硬截止，僅於正常速度時預篩
}
```

---

### 模組 ⑤ — 路線進度投影
**檔案：** `firmware/src/pipeline/projection.rs`

```rust
// t_num = dot(G - Pi, Pi+1 - Pi)，升型至 i64
let t_num: i64 = dot_i64(gx - node.x_cm, gy - node.y_cm,
                          node.dx_cm,     node.dy_cm);
// 截斷至 [0, len2]
let t_clamped = t_num.clamp(0, node.len2_cm2);
// z_cm = cum_dist + t × seg_len / len2（整數除法，無 sqrt）
let z_cm = node.cum_dist_cm
    + ((t_clamped * node.seg_len_cm as i64) / node.len2_cm2) as i32;
```

單調性約束（最強穩定性保護）：

```rust
// 逆向跳點：拒絕此 GPS 樣本
if z_cm - self.s_prev < -1000 {
    return Err(ProjectionError::MonotonicViolation);
}
```

---

### 模組 ⑥ — 速度約束過濾
**檔案：** `firmware/src/pipeline/speed_filter.rs`

```rust
const V_MAX_CMS:   i32 = 1667; // 60 km/h
const SIGMA_GPS:   i32 = 2000; // 20 m GPS 裕度
const D_MAX:       i32 = V_MAX_CMS + SIGMA_GPS; // = 3667 cm（約 37 m）

pub fn is_jump(z_new: DistCm, s_prev: DistCm) -> bool {
    (z_new - s_prev).unsigned_abs() > D_MAX as u32
}
```

---

### 模組 ⑦ — 1D 卡爾曼濾波器
**檔案：** `firmware/src/pipeline/kalman.rs`

固定增益（無 HDOP）：

```rust
// Ks = 51/256 ≈ 0.20，Kv = 77/256 ≈ 0.30
self.s_cm  = s_pred + (51 * (z_cm      - s_pred)) / 256;
self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
```

自適應增益（HDOP 可用時）：

| HDOP 範圍 | Ks（分子/256） | 說明 |
|-----------|--------------|------|
| ≤ 2.0 | 77/256 ≈ 0.30 | 訊號佳，信任 GPS |
| 2.1–3.0 | 51/256 ≈ 0.20 | 一般市區（預設值） |
| 3.1–5.0 | 26/256 ≈ 0.10 | 較差，大幅降低 GPS 比重 |
| > 5.0 | 13/256 ≈ 0.05 | 接近 Dead-Reckoning，僅微修正 |

速度平滑（EMA，α = 0.3）：

```rust
// v_smooth = v_smooth × 7/10 + v_new × 3/10（整數近似）
self.v_smooth = (self.v_smooth * 7 + v_new * 3) / 10;
```

> **啟動暖機：** 系統上電後等待 **3 個 GPS 週期（3 s）** 再啟動到站判定，讓 Kalman Filter 收斂至合理初始狀態。

---

### 模組 ⑧ — 航位推算補償
**檔案：** `firmware/src/pipeline/dead_reckoning.rs`

```rust
pub struct DrState {
    pub s_last:        DistCm,  // 最後有效路線進度
    pub v_last:        SpeedCms,// 最後有效速度
    pub elapsed_ticks: u8,      // 已推算秒數
    pub is_active:     bool,
}

impl DrState {
    /// GPS 無效時呼叫（每秒一次）
    pub fn tick(&mut self) -> Option<DistCm> {
        if self.elapsed_ticks >= 10 { return None; } // 超過上限
        self.elapsed_ticks += 1;
        self.s_last += self.v_last; // Δt = 1s
        Some(self.s_last)
    }

    /// GPS 恢復後 soft correction
    pub fn resync(&mut self, z_gps: DistCm) {
        // s = s_dr × 0.8 + z_gps × 0.2
        self.s_last = (self.s_last * 8 + z_gps * 2) / 10;
        self.elapsed_ticks = 0;
        self.is_active = false;
    }
}
// 16 bytes SRAM
```

---

### 模組 ⑨ — 站點廊道過濾
**檔案：** `firmware/src/arrival/corridor.rs`

```rust
/// 線性掃描 50 站點，找到當前 s_cm 所在廊道（< 0.05 ms）
pub fn find_active_stop(s_cm: DistCm, stops: &[Stop]) -> Option<u8> {
    stops.iter().position(|stop| {
        s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
    }).map(|i| i as u8)
}
```

廊道設計：

- **前置寬度 L_pre = 8000 cm（80 m）**：提前進入偵測區
- **後置寬度 L_post = 4000 cm（40 m）**：確保離站判斷
- **最小分隔 δ_sep = 2000 cm（20 m）**：相鄰廊道重疊時取中間點截斷

---

### 模組 ⑩ — 到站概率模型
**檔案：** `firmware/src/arrival/probability.rs`

四特徵 Bayesian 融合：

| 特徵 | 計算方式 | 說明 |
|------|---------|------|
| F1 距離差 | `gaussian_lut(|s - stop.progress|, σ_d=2750)` | 越接近站點越高 |
| F2 速度 | `logistic_lut(v_cms, v_stop=200)` | 越慢越高 |
| F3 進度誤差 | `gaussian_lut(|s - stop.progress|, σ_p=2000)` | 精確進度匹配 |
| F4 停留時間 | `min(dwell_ticks × 255 / 10, 255)` | 停留 ≥ 10s → 255 |

```rust
/// 融合公式（初始權重 13:6:10:3，總和 32 = 2^5）
pub fn arrival_probability(f1: Prob8, f2: Prob8, f3: Prob8, f4: Prob8) -> Prob8 {
    ((13u32 * f1 as u32
    +  6u32 * f2 as u32
    + 10u32 * f3 as u32
    +  3u32 * f4 as u32) / 32) as u8
}

// 到站觸發：P > θ_arrival = 191（對應 255 × 0.75 ≈ 75%）
const THETA_ARRIVAL: Prob8 = 191;
```

---

### 模組 ⑪ — 到站狀態機
**檔案：** `firmware/src/arrival/state_machine.rs`

```
Approaching ──進入廊道 + d < 12000 cm──→ Approaching
Approaching ──d < 5000 cm──────────────────────────→ Arriving
Arriving    ──d < 3000 cm + v < 56 cm/s + P > 191──→ AtStop (ARRIVED event)
AtStop      ──d > 4000 cm + ŝ > s_i─────────────────→ Departed
Departed    ──next corridor──────────────────────────→ Approaching (next stop)
```

防護機制：
- **防重複：** `last_arrived` 記錄站序，同站不重複觸發
- **防跳站：** 僅允許 `active_stop_idx` 遞增，不允許向前跳站
- **並發安全：** 到站事件透過 `AtomicU32` 發布（見第 8 章）

---

### 模組 ⑫ — 站序復原演算法
**檔案：** `firmware/src/arrival/recovery.rs`

觸發條件：GPS 斷訊 > 10s 恢復後 / 系統重啟 / 站序跳躍 > 500 m

```rust
pub fn recover_stop_index(s_cm: DistCm, stops: &[Stop], last_idx: u8) -> Option<u8> {
    const SEARCH_RANGE:    i32 = 20_000; // ±200 m
    const MIN_GAP:         i32 =  5_000; // 確信門檻（50 m）
    const GUARD_MARGIN:    i32 =  5_000; // 進度保護裕度（50 m）
    const TRIGGER_JUMP:    i32 = 50_000; // 觸發門檻（500 m）

    let mut best_dist = i32::MAX;
    let mut best_idx  = last_idx;
    let mut second_best = i32::MAX;

    for (i, stop) in stops.iter().enumerate() {
        let dist = (stop.progress_cm - s_cm).abs();
        if dist > SEARCH_RANGE { continue; }
        // 進度保護：不允許向後復原
        if stop.progress_cm < s_cm - GUARD_MARGIN { continue; }
        if dist < best_dist {
            second_best = best_dist;
            best_dist   = dist;
            best_idx    = i as u8;
        } else if dist < second_best {
            second_best = dist;
        }
    }
    // 最優與次優差距不足時不更新（模糊）
    if second_best - best_dist < MIN_GAP { return None; }
    Some(best_idx)
}
```

---

## 6. Build Script（build.rs）

`build.rs` 在編譯期生成 LUT 二進位檔案，確保 LUT 與演算法參數永遠同步。

```rust
// firmware/build.rs
use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    // ── Gaussian LUT：256 bytes ──────────────────────────────────
    // 輸入 x/σ ∈ [0, 4.0)，步長 4/256 = 0.015625
    // 輸出 exp(-x²/2) × 255，存為 u8
    let gaussian: Vec<u8> = (0i32..256)
        .map(|i| {
            let x = i as f64 / 64.0; // x ∈ [0, 4.0)
            ((-x * x / 2.0).exp() * 255.0).round() as u8
        })
        .collect();
    fs::write(out.join("gaussian_lut.bin"), &gaussian).unwrap();

    // ── Logistic LUT：128 bytes ──────────────────────────────────
    // 輸入 v ∈ [0, V_MAX=1667 cm/s]
    // 輸出 1/(1+exp(k(v−v0))) × 255，v0=200 cm/s，k=0.05
    let logistic: Vec<u8> = (0i32..128)
        .map(|i| {
            let v  = i as f64 * 1667.0 / 127.0;
            let k  = 0.05_f64;
            let v0 = 200.0_f64;
            (255.0 / (1.0 + (k * (v - v0)).exp())).round() as u8
        })
        .collect();
    fs::write(out.join("logistic_lut.bin"), &logistic).unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}
```

### 6.1 Firmware 中載入 LUT

```rust
// firmware/src/lut.rs
use crate::types::{DistCm, SpeedCms, Prob8};

static GAUSSIAN_LUT: &[u8; 256] =
    include_bytes!(concat!(env!("OUT_DIR"), "/gaussian_lut.bin"));

static LOGISTIC_LUT: &[u8; 128] =
    include_bytes!(concat!(env!("OUT_DIR"), "/logistic_lut.bin"));

/// 距離 Gaussian 概率：exp(-d²/(2σ²)) → Prob8
/// 查表耗時 < 3 cycles，256 bytes Flash
pub fn gaussian_lut(d_cm: DistCm, sigma_cm: DistCm) -> Prob8 {
    let idx = ((d_cm.unsigned_abs() as i64 * 64) / sigma_cm as i64)
              .min(255) as usize;
    GAUSSIAN_LUT[idx]
}

/// 速度 Logistic 概率：低速高值，高速低值 → Prob8
pub fn logistic_lut(v_cms: SpeedCms) -> Prob8 {
    let idx = ((v_cms as i64 * 127) / 1667).clamp(0, 127) as usize;
    LOGISTIC_LUT[idx]
}
```

---

## 7. Flash 資料存取（XIP）

### 7.1 靜態資料連結

Pico 2 支援從 Flash 直接執行（XIP），路線資料不需複製至 SRAM：

```rust
// firmware/src/route_data.rs
#[link_section = ".rodata"]
static ROUTE_DATA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/route_data.bin"));

/// 零拷貝取得 RouteNode 切片（Flash XIP 直接讀取）
pub fn route_nodes() -> &'static [RouteNode] {
    let ptr   = ROUTE_DATA.as_ptr() as *const RouteNode;
    let count = ROUTE_DATA.len() / core::mem::size_of::<RouteNode>();
    // SAFETY: ROUTE_DATA 為 repr(C) 結構體，對齊由 linker script 保證
    unsafe { core::slice::from_raw_parts(ptr, count) }
}
```

### 7.2 route_data.bin 佈局

離線工具（`preprocessor/src/pack.rs`）將所有資料序列化為單一 .bin 檔：

| 區段 | 大小 | 說明 |
|------|------|------|
| magic（4 bytes） | 4 B | `0x42555341`（ASCII "BUSA"） |
| version（u16） | 2 B | 格式版本（v8.7 = 4） |
| node_count（u16） | 2 B | RouteNode 數量 |
| stop_count（u8） | 1 B | Stop 數量 |
| grid_origin（8 B） | 8 B | x0_cm, y0_cm（i32 × 2） |
| route_nodes（N × 32 B） | ~19.2 KB | RouteNode 陣列（v8.7） |
| stops（M × 12 B） | ~0.6 KB | Stop 陣列 |
| grid_index | ~1.2 KB | Spatial Grid Index |
| CRC32（4 bytes） | 4 B | 整體完整性驗證 |
| **合計** | **~22 KB** | |

> **v8.7 更新：** VERSION=4，RouteNode 從 40→32 bytes，總體從 ~34 KB 降至 ~22 KB（節省 12 KB Flash）。

### 7.3 啟動完整性驗證

```rust
fn verify_route_data() -> Result<(), &'static str> {
    let data = route_data::raw_bytes();
    if &data[..4] != b"BUSA" {
        return Err("invalid magic");
    }
    let (body, crc_bytes) = data.split_at(data.len() - 4);
    let expected = u32::from_le_bytes(crc_bytes.try_into().unwrap());
    if crc32(body) != expected {
        return Err("route_data CRC mismatch");
    }
    Ok(())
}
```

---

## 8. 並發安全與 Atomic 使用

RP2350 為雙核心架構，`current_stop_index` 可能跨 Core 或 IRQ 存取，必須使用 Atomic 操作：

```rust
// firmware/src/arrival/state_machine.rs
use core::sync::atomic::{AtomicU32, Ordering};

/// 當前已到達站點索引（跨 Core / IRQ 安全）
pub static CURRENT_STOP: AtomicU32 = AtomicU32::new(0);

/// 更新到站事件（在 GPS loop Core 0 呼叫）
pub fn emit_arrival(stop_idx: u8) {
    CURRENT_STOP.store(stop_idx as u32, Ordering::Release);
}

/// 讀取當前站序（可在 Core 1 或其他 IRQ 中呼叫）
pub fn get_current_stop() -> u8 {
    CURRENT_STOP.load(Ordering::Acquire) as u8
}
```

並發注意事項：

- GPS pipeline 在 **Core 0** 執行（1 Hz loop），使用 `Ordering::Release / Acquire` pair
- `KalmanState` 與 `StopState` 僅在 Core 0 存取，不需 Atomic
- 若使用 UART IRQ 接收 GPS NMEA，需以 `critical_section` 保護 GPS 緩衝區
- 可使用 RP2350 SIO 硬體 FIFO 在 Core 0/1 之間傳遞到站事件

---

## 9. Cargo.toml 設定

### 9.1 Workspace Cargo.toml

```toml
[workspace]
members = ["firmware", "preprocessor", "shared"]
resolver = "2"
```

### 9.2 Firmware Cargo.toml

```toml
[package]
name    = "bus-arrival-firmware"
version = "0.1.0"
edition = "2021"

[dependencies]
rp235x-hal     = { version = "0.2", features = ["rt"] }
cortex-m       = { version = "0.7", features = ["critical-section-single-core"] }
cortex-m-rt    = "0.7"
critical-section = "1.1"
heapless       = "0.8"    # 固定大小資料結構，no_std 相容
defmt          = "0.3"    # 嵌入式除錯日誌（可選）
defmt-rtt      = "0.4"    # RTT 輸出（可選）

[profile.release]
opt-level     = "s"       # 優化大小（比 z 稍快）
lto           = true
codegen-units = 1
debug         = 2         # 保留除錯符號（probe-rs 用）

[[bin]]
name  = "firmware"
test  = false
bench = false
```

### 9.3 Memory.x（RP2350）

```
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 520K
}

SECTIONS {
    /* 路線資料放在 .rodata，Flash XIP 直接讀取 */
    .rodata : {
        *(.rodata .rodata.*)
    } > FLASH
}
```

---

## 10. 效能預算

### 10.1 每次 GPS 更新計算成本（1 Hz，無 FPU）

| 模組 | 耗時估計 | 主要操作 |
|------|---------|---------|
| Spatial Grid Index | < 0.1 ms | 整數 grid lookup |
| Map Matching（含 heading） | < 0.5 ms | ~10 路段 × i64 d² + heading filter |
| Segment Projection | < 0.1 ms | 1 次 i64 點積 |
| Speed Constraint | < 0.05 ms | 1 次 i32 比較 |
| Kalman Filter（1D 整數增益） | < 0.2 ms | 4 次整數乘加 |
| Dead-Reckoning | < 0.1 ms | 1 次整數乘加 |
| Stop Corridor Check | < 0.05 ms | 2 次 i32 比較 |
| Stop Probability Model | < 0.1 ms | 2 次 LUT + 加權求和 |
| Stop State Machine | < 0.05 ms | FSM match |
| Stop Index Recovery（觸發時） | < 0.5 ms | 50 次整數比較 |
| **合計（正常模式）** | **< 1.5 ms** | **CPU < 8%（@150 MHz）** |

### 10.2 記憶體佔用

| 資料 / 狀態 | 佔用 | 位置 |
|------------|------|------|
| 路線資料 + LUT | ~34 KB | Flash（XIP） |
| KalmanState | 8 bytes | SRAM |
| Dead-Reckoning State | 16 bytes | SRAM |
| StopState | ~50 bytes | SRAM |
| Persisted State buffer | 12 bytes | SRAM |
| GPS 緩衝區 + 速度歷史 | < 256 bytes | SRAM |
| **SRAM 合計** | **< 1 KB** | runtime |

### 10.3 準確率里程碑

| 版本 | 目標準確率 | 新增功能 |
|------|----------|---------|
| M1：純距離閾值 | ~80% | corridor check only |
| M2：+ 速度約束 | ~88% | speed filter |
| M3：+ Heading Map Matching | ~93% | heading ramp + grid index |
| M4：+ 1D Kalman Filter | ~96% | kalman + dead-reckoning |
| **M5：完整 Pipeline** | **≥ 97%** | probability model + state machine + recovery |

---

## 11. 測試策略

### 11.1 單元測試（Host 環境，無需硬體）

```rust
// firmware/src/pipeline/kalman.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kalman_converges_at_constant_speed() {
        let mut k = KalmanState { s_cm: 0, v_cms: 0 };
        // 模擬公車以 1000 cm/s 匀速行駛 30 秒
        for t in 0..30i32 {
            k.update(t * 1000, 1000);
        }
        // 收斂後速度估計應接近 1000 cm/s（誤差 < 50 cm/s）
        assert!((k.v_cms - 1000).abs() < 50,
            "speed did not converge: {}", k.v_cms);
    }

    #[test]
    fn speed_filter_catches_jump() {
        let s_prev: DistCm = 50_000;
        let z_jump: DistCm = 150_000; // 瞬移 100 m，遠超 D_max = 3667 cm
        assert!(is_jump(z_jump, s_prev),
            "jump not detected");
    }

    #[test]
    fn heading_weight_ramp() {
        assert_eq!(heading_weight(0),  0);   // 靜止無懲罰
        assert_eq!(heading_weight(83), 256); // 3 km/h 全效
        assert!(heading_weight(40) > 0 && heading_weight(40) < 256); // 漸進
    }
}
```

### 11.2 GPS Log 回放測試

```python
# tools/replay.py
# 輸入：GPS NMEA log + 真實到站標注
# 輸出：TP / FP / FN 統計，F1-score

import subprocess, json

result = subprocess.run(
    ['cargo', 'run', '--bin', 'replay', '--', 'gps_log.nmea'],
    capture_output=True, text=True
)
events = json.loads(result.stdout)
tp = sum(1 for e in events if e['predicted'] == 1 and e['label'] == 1)
fp = sum(1 for e in events if e['predicted'] == 1 and e['label'] == 0)
fn = sum(1 for e in events if e['predicted'] == 0 and e['label'] == 1)
precision = tp / (tp + fp + 1e-9)
recall    = tp / (tp + fn + 1e-9)
f1        = 2 * precision * recall / (precision + recall + 1e-9)
print(f"Precision={precision:.3f}  Recall={recall:.3f}  F1={f1:.3f}")
```

### 11.3 驗收標準

| 測試場景 | 通過標準 |
|---------|---------|
| 一般到站（GPS 良好） | True Positive Rate ≥ 97% |
| 錯誤觸發率 | False Positive Rate ≤ 2% |
| 近距離相鄰站（< 120 m） | 正確率 ≥ 95% |
| GPS 城市峽谷（HDOP > 4） | 正確率 ≥ 90% |
| GPS 斷訊 10 s 後恢復 | 站序正確，恢復後 < 2 s 同步 |

---

## 12. 重要參數速查

| 參數 | 整數值 | 實際意義 |
|------|--------|---------|
| Polyline 簡化容差 ε | 600–800 cm | Douglas-Peucker 一般路段 |
| 急彎保護容差 ε_curve | 200–300 cm | 轉彎角 > 2000 cdeg（20°）處 |
| 最大路段長度 | 2500–3000 cm | 超過則插入補充節點 |
| 站點保護範圍 | ±3000 cm | 強制保留節點 |
| Grid Cell 大小 Δg | 10000 cm | Spatial Index 格網（100 m） |
| Heading Ramp v_ramp | 83 cm/s | 3 km/h，heading 權重起點 |
| 方向硬截止 | 9000 cdeg | 90°，v > 83 cm/s 時才預篩 |
| GPS 雜訊裕度 σ_gps | 2000 cm | D_max 計算使用 |
| 最大車速 V_max | 1667 cm/s | 60 km/h，D_max = 3667 cm |
| Kalman Gain Ks（固定） | 51/256 ≈ 0.20 | s 更新增益 |
| Kalman Gain Kv | 77/256 ≈ 0.30 | v 更新增益 |
| EMA 係數 α_v | 3/10 ≈ 0.30 | 速度平滑 |
| DR 最大時限 | 10 s | Dead-Reckoning 上限 |
| DR 重同步 GPS 占比 | 2/10 ≈ 0.20 | soft correction |
| 廊道前置寬度 L_pre | 8000 cm | 80 m |
| 廊道後置寬度 L_post | 4000 cm | 40 m |
| 廊道最小分隔 δ_sep | 2000 cm | 20 m，相鄰廊道重疊保護 |
| Distance sigma σ_d | 2750 cm | Gaussian LUT，F1 特徵 |
| Progress sigma σ_p | 2000 cm | Gaussian LUT，F3 特徵 |
| Speed stop threshold v_stop | 200 cm/s | 7.2 km/h，Logistic LUT 中心 |
| Dwell time reference T_ref | 10 s | F4 停留時間特徵 |
| 到站概率閾值 θ_arrival | 191（u8） | 255 × 0.75 ≈ 75% 概率 |
| Recovery 搜索範圍 | ±20000 cm | ±200 m |
| Recovery 觸發門檻 | 50000 cm | 500 m 跳躍才觸發 |
| 啟動暖機時間 | 3 s | 3 個 GPS 週期，Kalman 收斂 |

---

## 13. 開發里程碑與 Checklist

### 離線預處理（preprocessor crate）

- [ ] `shared/src/lib.rs`：定義 `RouteNode`、`Stop`、`GridIndex`（含 `size_of` 編譯期斷言）
- [ ] `preprocessor/src/simplify.rs`：Douglas-Peucker + 急彎/站點保護
- [ ] `preprocessor/src/linearize.rs`：平面近似累積距離、全係數預算、廊道邊界計算（含重疊截斷）
- [ ] `preprocessor/src/grid_index.rs`：100 m 格網建立、路段索引分配
- [ ] `preprocessor/src/pack.rs`：序列化 → `route_data.bin`（magic + CRC32）
- [ ] 驗證：節點數 ~640，Flash ~34 KB，CRC 通過

### Firmware 定位 Pipeline

- [ ] `build.rs`：Gaussian LUT（256 B）+ Logistic LUT（128 B）生成
- [ ] `firmware/src/types.rs`：五個型別別名 + `dot_i64` 工具函式
- [ ] `firmware/src/route_data.rs`：Flash XIP 存取、啟動 CRC 驗證
- [ ] `firmware/src/lut.rs`：`gaussian_lut()`、`logistic_lut()`
- [ ] `pipeline/map_match.rs`：Grid lookup + heading ramp + i64 距離平方評分
- [ ] `pipeline/projection.rs`：segment projection + 單調性約束（-1000 cm 閾值）
- [ ] `pipeline/speed_filter.rs`：D_max = 3667 cm 跳點拒絕
- [ ] `pipeline/kalman.rs`：固定增益 update() + HDOP 自適應（可選）+ 速度 EMA
- [ ] `pipeline/dead_reckoning.rs`：tick() / resync()，10 s 上限

### Firmware 到站判定

- [ ] `arrival/corridor.rs`：廊道啟用條件，線性掃描 50 站點
- [ ] `arrival/probability.rs`：四特徵 Bayesian 融合，整數權重 13:6:10:3
- [ ] `arrival/state_machine.rs`：FSM 四狀態 + `AtomicU32` 到站事件 + 防重複/防跳站
- [ ] `arrival/recovery.rs`：站序復原，±200 m 搜索，500 m 觸發門檻

### 驗證與調校

- [ ] GPS log 回放：`replay.py`，目標 TP ≥ 97%、FP ≤ 2%
- [ ] 近距離站點（< 120 m）場景：正確率 ≥ 95%
- [ ] GPS 斷訊（10 s）場景：DR 補償正確，恢復後 < 2 s 同步
- [ ] 城市峽谷（HDOP > 4）場景：自適應 Kalman gain 啟用，正確率 ≥ 90%
- [ ] 附錄 B 調校：收集 ≥ 200 筆事件，Grid Search 優化融合權重

---

*依據 GPS-Based Bus Arrival Detection System 技術設計報告 v2.1*
