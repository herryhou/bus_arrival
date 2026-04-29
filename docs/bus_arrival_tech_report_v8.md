# GPS-Based Bus Arrival Detection System

## 技術設計報告：適用於 Raspberry Pi Pico 2 嵌入式平台之路線感知到站判定演算法

**目標受眾：** Embedded Rust 開發團隊  
**硬體平台：** Raspberry Pi Pico 2（RP2350）  
**文件版本：** v9.0（二層架構重構：控制/估計分離）

### 版本更新記錄（Changelog）

#### v8.10（2026-04-29）- 估計就緒與檢測門控分離
- 將單一 warmup 計數器分離為獨立的「估計就緒」與「檢測門控」
- 估計就緒控制 Kalman Filter 收斂與標題過濾器啟用
- 檢測門控獨立控制到站檢測啟用時機
- 兩者皆具備 3 tick 收斂路徑與 10 tick 超時安全閥

#### v8.9（2026-04-19）- 脫離路線檢測與 GPS 跳躍恢復

**新增功能：** 脫離路線檢測、位置凍結、GPS 跳躍恢復
- 5-tick 遲滯確認機制（避免 GPS 雜訊誤觸發）
- 脫離路線期間凍結 s_cm 進度估計
- GPS 恢復時自動跳至最近的前方站點（「重啟」行為）
- 支援繞路後的正確站點重新獲取

詳見 [Section 16](#16-脫離路線檢測與恢復模組---v89-新增) 完整說明。

---

#### v9.0（2026-04-28）- 二層架構重構：控制/估計分離

**架構重構：** 將嵌入式韌體重構為二層架構，實現關注點分離與單一職責原則
- **控制層（Control Layer）**：系統狀態機、模式轉換、偵測協調
- **估計層（Estimation Layer）**：隔離的 GPS → 位置管線（Kalman + DR）
- **恢復模組（Recovery Module）**：純函數恢復邏輯，明確輸入/輸出

**核心設計原則：**
- 隔離性：估計層僅維護內部狀態，無控制層存取權
- 統一觸發：所有模式轉換僅使用估計信號（`divergence_d2`、`z_gps_cm`）
- 單一轉換：每個 tick 最多一次模式轉換（防止競爭條件）
- 一等公民恢復：恢復是系統模式，非內聯邏輯

**空間契約（Spatial Contract）明確化：**
- F1（距離似然度）：使用 `z_gps_cm`（原始 GPS 投影空間）
- F3（進度似然度）：使用 `s_cm`（濾波路線空間）
- 此雙空間方法為有意設計，非歧義行為

詳見 [Section 22](#22-二層架構設計v90-新增) 完整說明。

---

#### v8.8 → v8.7（2026-04-13）- E1 修正：Filter-then-Rank 架構

**問題：** 原設計使用混合評分（$d^2 + \lambda \cdot \text{diff}^2 \cdot w_h$），但因 λ 缺失且單位不匹配（cm² + cdeg²），heading 項遠大於 distance 項（10,000×），實際上變成了硬排除。

**解決方案：** Filter-then-Rank 架構
- **Filter（過濾）**：布林 heading 閘（`heading_eligible`），基於速度調整的閾值（`heading_threshold_cdeg`）
- **Rank（排名）**：純 distance 平方（`segment_score`），移除 heading 參數

**主要變更：**
- 新增 `MAX_HEADING_DIFF_CDEG = 9_000`（90° heading 閘常數）
- 新增 `heading_threshold_cdeg(w)` 函數：速度依賴的 heading 閾值
- 新增 `heading_eligible(gps_heading, gps_speed, seg_heading)` 函數：布林過濾器
- 新增 `best_eligible()` 函數：雙追蹤器（eligible + any）
- 重寫 `find_best_segment_restricted()`：window → grid 搜索，雙追蹤器 seeding
- 簡化 `segment_score()`：移除 heading 參數，返回純 distance 平方

**關鍵不變量（Invariants）：**
- Early exit 需要 `eligible_found = true`
- `best_eligible_dist2 = MAX` 當 `window_eligible = false`（防止掩蓋 bug）
- 雙追蹤器從 window 傳承到 grid
- Grid 設定 `eligible_found = true` 當找到 eligible 路段
- Fallback 返回 `best_any_idx`（明確降級）

**測試更新：**
- 更新 `scenario_loop_closure` 測試以接受 filter-then-rank 行為（角落處多個 eligible 路段皆有效）

**檔案變更：**
- `crates/pipeline/gps_processor/src/map_match.rs`：核心實作
- `crates/pipeline/gps_processor/tests/bdd_localization.rs`：測試更新

---

## 摘要（Abstract）

本報告系統化整理一套適用於嵌入式車機環境之公車到站判定演算法架構。目標硬體為 Raspberry Pi Pico 2（RP2350，雙核 Cortex-M33，**無硬體 FPU**），GPS 更新頻率為 1 Hz（Δt = 1 s），已知完整路線 polyline 與停靠站 GPS 座標。

核心需求為解決 GPS 漂移、跳點（jump）、近距離站點混淆三類主要誤判場景，並支援**到站前語音播報**（提前 10–15 秒觸發）。本報告提出一套以確定性（deterministic）規則為基礎的工程化架構：以 Route Linearization 將問題降至一維，以**語義化整數型別（cm、0.01°、cm/s）取代浮點運算**以適應無 FPU 平台，以 Heading-Constrained Map Matching 進行路段篩選，以 1D Kalman Filter 平滑狀態估計，以 Dead-Reckoning 補償 GPS 斷訊，最終以 Stop Corridor（兼語音播報觸發）+ Probabilistic Arrival Model + Stop State Machine 三層機制完成到站判定。

完整 pipeline 在 Pico 2 上之計算成本估計為 **CPU < 8%、SRAM < 1 KB（runtime）**，可達到 **≥ 97% 到站判定準確率**，並具備 GPS 斷訊 10 秒以內之持續追蹤能力。路線資料（含預算係數）Flash 佔用約 **~10-12 KB**（v8.8 優化後）。

---

## 目錄

**第一部分：基礎與策略**
1. [背景與問題定義](#1-背景與問題定義)
2. [系統架構總覽](#2-系統架構總覽)
3. [整數運算策略](#3-整數運算策略integer-arithmetic-strategy)

**第二部分：離線預處理（模組 ①②③）**

4. [Polyline 簡化策略（模組 ①）](#4-polyline-簡化策略模組-)
5. [路線線性化（模組 ②）](#5-路線線性化模組-)
6. [空間格網索引（模組 ③）](#6-空間格網索引模組-)

**第三部分：定位 Pipeline（模組 ④–⑧）**

7. [方向約束地圖匹配（模組 ④）](#7-方向約束地圖匹配模組-)
8. [路線進度投影（模組 ⑤）](#8-路線進度投影模組-)
9. [速度約束過濾（模組 ⑥）](#9-速度約束過濾模組-)
10. [一維卡爾曼濾波器（模組 ⑦）](#10-一維卡爾曼濾波器模組-)
11. [航位推算補償（模組 ⑧）](#11-航位推算補償模組-)

**第四部分：到站判定（模組 ⑨–⑫）**

12. [站點廊道過濾（模組 ⑨）](#12-站點廊道過濾模組-)
13. [到站概率模型（模組 ⑩）](#13-到站概率模型模組-)
14. [到站狀態機（模組 ⑪）](#14-到站狀態機模組-)
15. [站序復原演算法（模組 ⑫）](#15-站序復原演算法模組-)

**第五部分：進階與總結**

16. [脫離路線檢測與恢復（模組 ⑬，v8.9 新增）](#16-脫離路線檢測與恢復模組---v89-新增)
17. [HMM 地圖匹配（進階選項）](#17-隱馬可夫模型地圖匹配hmm-map-matching進階選項)
18. [離線預處理完整流程](#18-離線預處理完整流程)
19. [效能摘要與資源評估](#19-效能摘要與資源評估)
20. [Embedded Rust 實作注意事項](#20-embedded-rust-實作注意事項)
21. [完整 Pipeline 總結](#21-完整-pipeline-總結)
22. [二層架構設計（v9.0 新增）](#22-二層架構設計v90-新增)
23. [測試案例與驗證](#23-測試案例與驗證)
- [附錄 A：參數快速參考](#附錄參數快速參考)
- [附錄 B：到站概率模型權重離線調校流程](#附錄-b到站概率模型權重離線調校流程)

---

## 1. 背景與問題定義

### 1.1 系統環境

本系統部署於公車車載設備（On-Board Unit, OBU），硬體平台為 Raspberry Pi Pico 2：

| 參數 | 數值 / 說明 |
|------|------------|
| MCU | RP2350（dual-core Arm Cortex-M33, 150 MHz） |
| SRAM | 520 KB（可用於 runtime 約 400–450 KB） |
| Flash | 2 MB 內建 Flash（路線資料預載） |
| FPU | **無硬體 FPU**（軟體浮點，比整數慢 3–5×） |
| GPS 更新率 | **1 Hz（Δt = 1 s）** |
| GPS 誤差 | ±5–30 m（市區），跳點可達 ±100 m |
| 已知資料 | 路線 polyline、所有停靠站 GPS 座標 |

> **備注：** RP2350 Cortex-M33 本身支援 FPU 選配，但 Pico 2 預設不啟用硬體 FPU。本文件以**無 FPU** 為基準，確保最保守情況下效能仍可接受。若電路板確認啟用硬體 FPU，第 3 章之整數運算策略可視情況調整，整體架構不變。

### 1.2 核心挑戰

單純使用「距離站點 < 50 m」作為到站判定條件，在以下場景中不夠 robust：

- **GPS 漂移（Drift）：** 城市峽谷（Urban Canyon）環境下，GPS 誤差常達 ±30 m，可能導致距離計算偏移而誤觸發或漏報。
- **GPS 跳點（Jump）：** 訊號中斷後恢復，座標可能瞬移超過 100 m，觸發錯誤的站點轉移。
- **近距離站點（Close Stops）：** 部分站距僅 80–120 m，GPS 誤差半徑已與站距相當，無法以單一距離閾值可靠區分。
- **GPS 斷訊（Outage）：** 高架橋、隧道、密集建築物可導致 5–10 秒無效 GPS 訊號，系統須持續維護位置估計。
- **無 FPU 效能限制：** Pico 2 軟體浮點運算（尤其 `sqrt`、`exp`、`cos`）較整數運算慢 3–5 倍。本系統以整數單位（cm、0.01°）直接表達所有物理量，runtime 不執行任何浮點運算（Kalman 協方差進階版除外）。

---

## 2. 系統架構總覽

完整 pipeline 分為三個階段：**離線預處理**（一次性，結果燒錄至 Flash）、**定位 Pipeline**（GPS loop，1 Hz）、**到站判定**（GPS loop，1 Hz）。

#### 離線預處理（PC/Server → Flash）
```txt
  ┌────────────────────────────────────────────────────────────┐
  │ Phase 1: OFFLINE PREPROCESSING (PC → Flash)                │
  ├────────────────────────────────────────────────────────────┤
  │ 原始路線 Polyline                                           │
  │      ↓                                                     │
  │ ① Polyline 簡化        ← Douglas-Peucker + 急彎/站點保護     │
  │      ↓                                                     │
  │ ② Route Linearization  ← 累積距離 D[i]、段係數、站點座標全預算  │
  │      ↓                                                     │
  │ ③ Spatial Grid Index   ← 路段空間索引，O(N) → O(k), k ≈ 5–10 │
  │      ↓                                                     │
  │    route_data.bin (~10-12KB Flash for v8.8)                            │
  └────────────────────────────────────────────────────────────┘
```
產物：route_data.bin（含 route_nodes / stops / grid_index）


#### 定位 Pipeline（GPS loop，1 Hz）— 載入 Flash 產物後執行
```txt
  ┌──────────────────────────────────────────────────────────────────┐
  │ Phase 2: LOCALIZATION PIPELINE (1Hz GPS Loop)                    │
  ├──────────────────────────────────────────────────────────────────┤
  │ 1Hz GPS Input → ④ Heading-Constrained Map Matching              │
  │                          (grid index + heading ramp weighting)   │
  │               → ⑤ Segment Projection (GPS → 1D route progress z)│
  │               → ⑥ Speed Constraint Filter (reject jumps >37m)   │
  │               → ⑦ 1D Kalman Filter (smooth ŝ, v̂)                │
  │               → ⑧ Dead-Reckoning (10s outage compensation)      │
  │                            ↓                                     │
  │                      Output: ŝ(t), v̂(t)                          │
  └──────────────────────────────────────────────────────────────────┘
```

#### 到站判定（GPS loop，1 Hz）
```txt
  ┌───────────────────────────────────────────────────────────────────┐
  │ Phase 3: ARRIVAL DETECTION (1Hz Loop)                             │
  ├───────────────────────────────────────────────────────────────────┤
  │ ŝ(t), v̂(t)                                                        │
  │    → ⑨ Stop Corridor Filter (80m pre/40m post)                   │
  │         └─ 廊道入口（corridor_start）觸發語音播報（首 tick 立即觸發）   │
  │    → ⑩ Stop Probability Model (4-feature weighted feature fusion)        │
  │    → ⑪ Stop State Machine (Approaching→Arriving→AtStop→Departed)  │
  │    → ⑫ Stop Index Recovery (post-outage resync)                  │
  │                   ↓                         ↓                     │
  │             Arrival Event Output      ANNOUNCE Event Output       │
  └───────────────────────────────────────────────────────────────────┘
```
---

## 3. 整數運算策略（Integer Arithmetic Strategy）

### 3.1 動機

Pico 2（RP2350 Cortex-M33，無硬體 FPU）執行軟體浮點運算的代價顯著：

| 運算類型 | 整數（週期） | 軟體浮點（週期） | 倍率 |
|---------|------------|----------------|------|
| 加法 / 乘法 | 1–2 | 5–15 | 3–8× |
| `sqrtf()` | — | 60–100 | — |
| `expf()` | — | 80–150 | — |
| `cosf()` | — | 50–80 | — |

本系統的對策是：**在語義上選擇合適的整數單位（cm、0.01°、cm/s），讓所有 runtime 計算均以純整數完成**。僅在兩個特定場景使用 LUT 替代浮點函數（`expf` → Gaussian LUT、`1/(1+e^x)` → Logistic LUT），Kalman 協方差進階版則使用 `f32`（僅 4 個變數，1 Hz 下可接受）。預計 **CPU 佔用降低 30–50%**，數值行為完全可預測。

> **不使用 Q16.16 定點數格式的原因：** 本專案的數值尺度（距離 cm、角度 0.01°、速度 cm/s）天然適合直接以整數表示，Q 格式只會增加移位操作與閱讀負擔，語義反而不清晰。

### 3.2 整數型別規範

```rust
// ✅ 語義清晰，單位即文件
type DistCm   = i32;  // centimeters，範圍 ±21 km，足夠
type SpeedCms = i32;  // cm/s
type GeoCdeg  = i16;  // 0.01 degrees，for lat/lon
type HeadCdeg = i16;  // 0.01 degrees，for heading
type Prob8    = u8;   // 0..255 表示 0.0–1.0，精度 1/256 已足
type Dist2    = i64;  // cm²，距離平方中間計算（避免溢位）
```

| 型別 | 單位 | 範圍 | 用途 |
|------|------|------|------|
| `DistCm` / `i32` | cm | ±21 km | `cum_dist`、`stop_progress`、dx/dy |
| `SpeedCms` / `i32` | cm/s | 0–6000 | 等同 0–216 km/h |
| `GeoCdeg` / `i16` | 0.01° | −180°–+180° | `lat_cdeg`、`lon_cdeg` |
| `HeadCdeg` / `i16` | 0.01° | −180°–+180° | `heading`、`theta_route`、diff |
| `Prob8` / `u8` | 1/256 | 0–255 | 到站概率、LUT 輸出 |
| `Dist2` / `i64` | cm² | — | 點積、距離平方中間值 |

### 3.3 點積溢位保護

距離以 `i32` cm 表示，點積運算需升至 `i64` 避免溢位（兩個 ±2×10⁶ cm 相乘 → 需 i64）：

```rust
fn dot_i64(ax: i32, ay: i32, bx: i32, by: i32) -> i64 {
    (ax as i64) * (bx as i64) + (ay as i64) * (by as i64)
}
```

### 3.4 `expf()` 的 LUT 替代方案

Stop Probability Model 中出現 $\exp(-x^2/2\sigma^2)$ 的計算，改以 256 項查找表（LUT）實現，輸出為 `u8`：

```rust
/// Gaussian LUT: normalized x/sigma ∈ [0, 4.0) → exp(-x²/2), scaled to u8 (0..255)
/// Generated at compile time in build.rs.
static GAUSSIAN_LUT: [u8; 256] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/gaussian_lut.bin"));

/// Returns exp(-d²/2σ²) as u8
pub fn gaussian_lut(d_cm: i32, sigma_cm: i32) -> u8 {
    let idx = ((d_cm.unsigned_abs() as i64 * 64) / sigma_cm as i64).min(255) as usize;
    GAUSSIAN_LUT[idx]
}
```

LUT 記憶體佔用：256 × 1 byte = **256 bytes**，查表耗時 < 3 cycles。

### 3.5 角度計算（無三角函數）

Heading 相似度篩選以純整數比較實現，完全避免 `cosf`：

```rust
/// Returns true if |heading_diff| <= threshold (unit: 0.01°)
pub fn heading_within(a: HeadCdeg, b: HeadCdeg, threshold: HeadCdeg) -> bool {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    let diff = if diff > 18000 { 36000 - diff } else { diff };
    diff <= threshold as u32
}
```

### 3.6 整體效益

| 指標 | 浮點版本 | 整數版本 |
|------|---------|---------|
| 每次 GPS 更新 CPU 時間（估計） | ~8–12 ms | ~1.5–2 ms |
| 數值行為 | 浮點累積，難以預測 | 完全可預測 |
| 程式碼可讀性 | 單位隱含 | 單位即型別名稱 |
| 額外 Flash 開銷 | 0 | +384 bytes（兩張 LUT，編譯期生成） |

---

## 4. Polyline 簡化策略（模組 ①）

### 4.1 問題

直接使用地圖 API 回傳之原始 polyline，節點間距通常為 1–2 m，一條 12 km 路線可達 7,000–8,000 節點。含預算係數後約 344 KB，超出 Flash 預算，且相鄰節點過密會造成投影計算抖動。**Polyline 簡化是離線預處理的第一步**，其輸出作為 Route Linearization（模組 ②）的輸入。

### 4.2 Douglas-Peucker 演算法

Douglas-Peucker 演算法（Ramer-Douglas-Peucker, RDP）遞迴地移除偏離直線距離小於容差 $\varepsilon$ 的中間節點。節點 $Q$ 到直線 $(P_1, P_2)$ 之距離公式：

$$d = \frac{|A \cdot x + B \cdot y + C|}{\sqrt{A^2 + B^2}}$$

**推薦參數：** $\varepsilon = 6$–$8\ \text{m}$，可將 7,200 節點簡化至約 600–900 節點，Flash 佔用降低 87%。

### 4.3 最大段長約束（自適應分段）

簡化後若存在長度 > 100 m 的路段，需在中間插入補充節點，以防止後續 GPS 投影時 progress 突然跳動。

**自適應分段策略（v8.6 新增）：**
- **一般路段：** 最大段長 100 m（10000 cm）
- **關鍵區域：** 最大段長 30 m（3000 cm）
  - 站點 ±100 m 範圍內
  - 急彎處（轉向角 > 20°）

此策略在保持到站檢測精度的同時，大幅減少一般路段的節點數量，降低 Flash 佔用。

### 4.4 曲線保護規則

簡化過程中需保護兩類區域，防止幾何失真：

- **站點錨定保護：** 站點 ±30 m 範圍內強制保留所有節點，確保站點附近投影精度。
- **急彎保護：** 路段轉彎半徑 < 50 m 處，降低 Douglas-Peucker 容差至 $\varepsilon_\text{curve} = 2$–$3\ \text{m}$，防止彎道幾何失真導致 Map Matching 誤判。轉彎角判斷：對相鄰三點，若方向差 $> 20°$ 則視為急彎。

### 4.5 效果

| 指標 | 優化前 | 優化後（v8.7） |
|------|--------|----------------|
| 節點數 | 7,200 | ~640 |
| Flash 佔用 | ~86 KB | **~18 KB** |
| Runtime 幾何重算 | 每次需算 A/B/C/len2 | 零重算（全預載，len2 改為 runtime 計算） |
| Map Matching 穩定性 | 抖動明顯 | 顯著改善 |

---

## 5. 路線線性化（模組 ②） {#5-路線線性化模組-}

### 5.1 概念

路線線性化是整個系統最重要的基礎轉換。其核心思想是：將二維地理空間中的路線 polyline 轉換為一維累積距離座標系，使所有後續計算均在此一維空間中進行。

定義 route coordinate（路線座標）為 $s$，其物理意義為「從路線起點沿路線行駛的距離（單位：**公分**）」。所有站點位置、車輛位置均以此座標表示。

### 5.2 累積距離計算

給定簡化後的 polyline 節點序列 $P_0, P_1, \ldots, P_N$，各節點之累積距離定義為：

$$D[0] = 0$$

$$D[i] = D[i-1] + \|P_i - P_{i-1}\|$$

段距離採用**平面近似（Planar Approximation）**：

$$d(P_i, P_{i+1}) = \sqrt{(\Delta x)^2 + (\Delta y)^2}$$

其中 $\Delta x = (\text{lon}_2 - \text{lon}_1) \cdot \cos(\text{lat}_\text{avg}) \cdot R$，$\Delta y = (\text{lat}_2 - \text{lat}_1) \cdot R$，$R = 6{,}371{,}000\ \text{m}$，結果換算為**公分（cm）**儲存為 `i32`。

**為何不採用 Haversine 公式？**  Haversine 是球面精確解，計算更複雜，但在本應用場景中不帶來實質改善。原因如下：

平面近似的誤差量級為 $\varepsilon \approx d^2 / (2R)$，在關鍵區域最大段長 30 m、台灣緯度（約 25°）條件下：

$$\varepsilon_\text{per segment} \approx \frac{30^2}{2 \times 6{,}371{,}000} \approx 0.00007\ \text{m} = 0.007\ \text{cm}$$

一條 12 km 路線（600 段）的累積誤差仍在 **< 5 cm** 量級，而 GPS 本身誤差為 ±500–3000 cm。兩者相差**四到五個數量級**，平面近似完全不是精度瓶頸。此外，計算結果最終存入 `i32` cm（精度 1 cm），Haversine 帶來的微小額外精度也無從體現。

> `sqrt` 僅在**離線預處理**時呼叫，不出現在 runtime hot path。Runtime 中距離比較統一使用**距離平方**（`i64`），完全避免 `sqrt`。

### 5.3 離線預算所有幾何係數

為確保 runtime 中零幾何重算，每個路段的所有所需係數均在離線階段預算完畢並儲存。v8.7 透過移除可由 runtime 廉價計算的係數來優化空間：

| 係數 | 型別 | 儲存位置 | 說明 |
|------|------|----------|------|
| `dx_cm`, `dy_cm` | `i16` | **Flash** | 段向量 $P_{i+1} - P_i$（cm），最大 ±100m |
| `seg_len_mm` | **`i32`** | **Flash** | 段長（mm，10× 精度）。最大支援 ±2147 km |
| `heading_cdeg` | `i16` | **Flash** | 段方向角（0.01°） |
| `cum_dist_cm` | `i32` | **Flash** | 起點至該節點的累積距離（cm） |
| `len2_cm2` | `i64` | **Runtime** | $\|P_{i+1}-P_i\|^2$（cm²）。**計算時須先轉型為 i64** |
| `seg_len_cm` | `i32` | **Runtime** | 段長（cm），由 $\text{seg\_len\_mm}/10$ 獲得 |

**備註：** `line_a`、`line_b`、`line_c` 係數已於 v8.2 移除。`len2_cm2` 已於 v8.7 移除 Flash 儲存，改為 runtime 計算，每節點節省 8 bytes。`seg_len_mm` 於 v8.7 從 `i64` 改為 `i32` 儲存，進一步節省 4 bytes。

### 5.4 站點座標編碼

對每個停靠站 $S$，找到其最近的 polyline segment $(P_i, P_{i+1})$，計算投影偏移量 $\delta$：

$$s_\text{stop} = D[i] + \delta \cdot (\text{seg\_len\_mm} / 10)$$

此編碼確保站點座標嚴格單調遞增，站序永遠正確。

---

## 5.5 RouteNode 結構與佈局 (v8.7) {#55-routenode-結構優化v87}

v8.7 版本進一步優化了 `RouteNode` 結構體，從 **40 bytes** 縮減至 **24 bytes**（40% 空間節省），同時提升了長度解析度。

### 新結構佈局

```rust
/// v8.7 優化後的 RouteNode 結構體（24 bytes）
/// repr(C) 確保與二進位格式的一致性
#[repr(C)]
pub struct RouteNode {
    // ── i32 fields first (4-byte aligned) ──────────────────────────
    pub x_cm: i32,             // X coordinate in cm
    pub y_cm: i32,             // Y coordinate in cm
    pub cum_dist_cm: i32,      // Cumulative distance in cm
    pub seg_len_mm: i32,       // Segment length in millimeters (10× precision)
    // ── i16 fields (2-byte aligned) ────────────────────────────────
    pub dx_cm: i16,            // Segment vector X (cm), max ±100m fits in i16
    pub dy_cm: i16,            // Segment vector Y (cm), max ±100m fits in i16
    pub heading_cdeg: i16,     // Heading in 0.01°
    pub _pad: i16,             // Alignment padding
}

// 編譯期驗證 — 確保結構體大小為 24 bytes
const _: () = assert!(core::mem::size_of::<RouteNode>() == 24);
```

### 記憶體佈局表格

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | x_cm | i32 | X coordinate in cm |
| 4 | y_cm | i32 | Y coordinate in cm |
| 8 | cum_dist_cm | i32 | Cumulative distance in cm |
| 12 | seg_len_mm | i32 | Segment length in millimeters |
| 16 | dx_cm | i16 | Segment vector X (cm) |
| 18 | dy_cm | i16 | Segment vector Y (cm) |
| 20 | heading_cdeg | i16 | Heading in 0.01° |
| 22 | _pad | i16 | Alignment padding |

**總大小：** 24 bytes（16 bytes i32 + 8 bytes i16）

### 關鍵變更（相較於 v8.5）

1. **移除 `len2_cm2` (i64)**
   - 原本儲存路段長度的平方值（cm²）
   - 現在 runtime 計算：`(seg_len_mm / 10)^2`
   - 節省 8 bytes

2. **升級 `seg_len_cm` (i32) → `seg_len_mm` (i32)**
   - 解析度提升 10 倍：從 cm 到 mm
   - 減少累積誤差，提升投影精度
   - 使用 i32 而非 i64，節省 4 bytes
   - i64 確保足夠的數值範圍（21 km 的 mm 值仍安全）

3. **縮減 `dx_cm`、`dy_cm` 從 i32 → i16**
   - 依據最大段長約束（100 m = 10,000 cm）
   - i16 範圍 ±32,767，足夠容納最大路段向量
   - 各節省 2 bytes，共 4 bytes

### 效益分析

| 指標 | v8.5 | v8.7 | 改善 |
|------|------|------|------|
| 結構體大小 | 40 bytes | 24 bytes | **-40%** |
| 600 節點 Flash 佔用 | 24 KB | 19.2 KB | **-4.8 KB** |
| 長度解析度 | 1 cm | 1 mm | **10× 提升** |
| Runtime 計算開銷 | 基準 | +1 次乘法 | < 0.1 ms |

### Runtime 相容性

v8.7 的 runtime 代碼需小幅修改以支援新的結構體：

```rust
// 計算 len2（runtime）
fn seg_len2_cm2(node: &RouteNode) -> i64 {
    let seg_len_cm = node.seg_len_mm / 10;
    seg_len_cm * seg_len_cm
}

// 投影計算（使用 i16 向量）
let dx = node.dx_cm as i64;  // 自動擴展
let dy = node.dy_cm as i64;
```

### 二進位格式版本

v8.8 使用 **VERSION 5**，與 v8.7（VERSION 4）不相容。舊版 `route_data.bin` 需重新生成。

**v8.8 Grid 優化：**
- **Bitmask 索引**：1 bit per cell，過濾空單元格
- **u16 偏移量**：僅非空 cell 儲存偏移（max 65,535）
- **空間節省**：~16 KB → ~5 KB（60-70% 壓縮）

> **完整二進制格式規格：** 參見 **[spatial_grid_binary_format.md](spatial_grid_binary_format.md)** - 包含 Grid 與 RouteNode 的完整 on-disk 佈局、讀寫實作細節，以及 XIP 支援說明。

---

## 6. 空間格網索引（模組 ③）

### 6.1 動機

原始 Map Matching 需對所有路段進行距離計算，複雜度為 $O(N)$。路線含 600 路段時，每次 GPS 更新需執行 600 次計算。透過格網索引可降至 $O(k)$，$k \approx 5$–$15$。

### 6.2 Fixed Grid 方法

將路線覆蓋區域劃分為固定大小的格網（grid），格網大小 $\Delta g = 100\ \text{m}$（= 10,000 cm）。

給定 GPS 點的投影座標 $(x_\text{cm}, y_\text{cm})$，其格網索引為：

$$g_x = \left\lfloor \frac{x_\text{cm} - x_{0,\text{cm}}}{\Delta g_\text{cm}} \right\rfloor, \qquad g_y = \left\lfloor \frac{y_\text{cm} - y_{0,\text{cm}}}{\Delta g_\text{cm}} \right\rfloor$$

其中 $(x_0, y_0)$ 為**路線 bounding box 的左下角**（即所有節點中最小的 x 與最小的 y），在離線預處理時計算並存入 Flash，用於將絕對座標轉換為從 0 起算的格網索引：

```rust
// Computed offline from all RouteNode coordinates:
pub struct GridOrigin {
    pub x0_cm: i32,  // min x of all route nodes
    pub y0_cm: i32,  // min y of all route nodes
}
```

Runtime 搜索範圍縮小至以 $(g_x, g_y)$ 為中心之 3×3 鄰域格網。

### 6.3 速度自適應搜索窗口

傳統固定 ±3 路段的候選窗口改為**速度自適應窗口**：

$$W = \left\lceil \frac{\hat{v}_\text{cm/s} \cdot \Delta t}{\overline{L}_\text{seg}} \right\rceil + 2$$

其中 $\Delta t = 1\ \text{s}$，$\overline{L}_\text{seg}$ 為路段平均長度（cm）。物理意義：車速越快，每次更新可能跨越的路段數越多，窗口動態加大；停站時窗口縮至最小值 2，減少無效計算。

**記憶體佔用（v8.8 優化）：**

使用 **稀疏格網 (Sparse Grid)** 格式：
- **Bitmask**: 1 bit per cell，標記非空單元格
- **u16 offsets**: 僅非空 cell 儲存偏移量（max 65,535 bytes）
- **Cell data**: count (u16) + segment indices (u16 each)

典型路線（60×60 grid，20-30% 非空）：
- Bitmask: 450 bytes (3600 bits)
- Offsets: ~720-1080 × 2 bytes = 1.4-2.1 KB
- Cell data: ~3-5 KB
- **總計: ~5-8 KB**（v8.7 為 ~16 KB）

**效能影響：**
- 查詢時增加一次 bitmask 檢查 + popcount 計算
- CPU 成本 < 0.01 ms（ARM Cortex-M33）

**詳細技術規格：**

參見 **[spatial_grid_binary_format.md](spatial_grid_binary_format.md)** - 完整的格網二進制格式說明，包括：
- In-memory vs on-disk 結構對比
- Byte-level 佈局範例
- 讀取/寫入實作細節
- XIP (eXecute In Place) 支援說明

---

## 7. 方向約束地圖匹配（模組 ④）

### 7.1 候選路段距離計算（無 `sqrt`）

對每個候選路段 $(P_i, P_{i+1})$，計算 GPS 點 $G$ 到該路段之**距離平方**（runtime 完全避免 `sqrt`）。

**實作方法：點積投影法（Dot Product Projection）**

此方法將 GPS 點投影至路段上，限制投影點在路段範圍內，再計算距離平方。對線段邊界的處理直觀（投影點 clamp 至路段範圍），且與線性距離公式在數學上等價。

$$\Delta_x = G_x - P_{i,x}, \qquad \Delta_y = G_y - P_{i,y}$$

$$t_\text{num} = \Delta_x \cdot \text{dx}_\text{cm} + \Delta_y \cdot \text{dy}_\text{cm} \qquad \text{（i64 點積）}$$

$$t = \text{clamp}(t_\text{num},\; 0,\; \text{len2}_\text{cm2})$$

投影點座標：

$$P_t = P_i + \frac{t}{\text{len2}_\text{cm2}} \cdot (\text{dx}_\text{cm},\; \text{dy}_\text{cm})$$

距離平方：

$$d^2(G,\; \text{seg}_i) = \|G - P_t\|^2$$

其中 $\text{dx}_\text{cm}$、$\text{dy}_\text{cm}$、$\text{seg\_len\_mm}$ 已儲存於 `RouteNode`。**$\text{len2}_\text{cm2}$ 則由 runtime 計算：$(\text{seg\_len\_mm} / 10)^2$**。Runtime 僅需整數運算：點積（i64）、除法（i64）、距離平方（i64），完全避免 `sqrt`。

### 7.2 方向篩選（Filter-then-Rank 架構）

**E1 修正：** 原設計使用混合評分（$d^2 + \lambda \cdot \text{diff}^2 \cdot w_h$），但因單位不匹配（cm² + cdeg²）且 λ 缺失，導致 heading 項遠大於 distance 項（10,000×），實際上變成了硬排除。

**新設計：Filter-then-Rank**

採用兩階段架構解決單位混合問題：
1. **Filter（過濾）**：布林 heading 閘，基於速度調整的閾值
2. **Rank（排名）**：純 distance 平方（$d^2$）

這種架構的優點：
- 物理可解釋：heading 閘值單位為度（°），任何開發者都能理解
- 避免單位混合：不將 cm² 和 cdeg² 相加
- 清晰的責任分離：heading 判斷可行性，distance 判斷優劣

#### 7.2.1 Heading Weight Ramp（速度漸進權重）

定義方向權重係數 $w_h \in [0, 256]$（整數 1/256 scale）：

$$w_h = \min\!\left(\frac{v_\text{cms}}{v_\text{ramp}},\; 1\right) \times 256, \quad v_\text{ramp} = 83\ \text{cm/s（3 km/h）}$$

整數實作：

```rust
/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h).
fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}
```

| 速度 | 權重 | 含義 |
|------|------|------|
| 0 cm/s | 0 | 停車，heading 不可靠 |
| 40 cm/s（~1.4 km/h） | ~123 | 部分約束 |
| 83 cm/s（3 km/h） | 256 | 完全約束 |
| > 83 cm/s | 256 | 完全約束 |

#### 7.2.2 Heading Threshold（速度依賴閾值）

Heading 閘值隨速度線性插值，在停止時完全開放（因 heading 不可靠），在正常速度時收緊至 90°：

```rust
/// Heading filter threshold for a given speed weight.
/// Returns u32::MAX (gate disabled) when w = 0 — stopped, heading unreliable.
/// Returns 90° (9000 cdeg) at w = 256 — full speed, meaningful gate.
fn heading_threshold_cdeg(w: i32) -> u32 {
    if w == 0 {
        return u32::MAX;
    }
    // threshold = 36000 - (36000 - 9000) × w / 256
    let range = 36_000u32 - 9_000; // 27 000
    36_000 - range * w as u32 / 256
}
```

- **w = 0（停止）**：`u32::MAX` → 閘完全開放
- **w = 128（~1.5 km/h）**：約 22,500 cdeg（225°，幾乎開放）
- **w = 256（≥3 km/h）**：9,000 cdeg（90°，有意義的閘）

#### 7.2.3 Heading Eligible（布林過濾器）

```rust
/// Returns true if segment is a plausible direction of travel.
/// Three cases:
///   - Sentinel heading (i16::MIN): GGA-only mode → always eligible
///   - Stopped (w = 0): heading unreliable → always eligible  
///   - Moving: eligible iff heading_diff ≤ threshold(speed)
fn heading_eligible(gps_heading: HeadCdeg, gps_speed: SpeedCms, seg_heading: HeadCdeg) -> bool {
    if gps_heading == i16::MIN {
        return true; // GGA-only: preserve existing sentinel behaviour
    }
    let w = heading_weight(gps_speed);
    let threshold = heading_threshold_cdeg(w);
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as u32;
    diff <= threshold
}
```

**關鍵設計決策：**
- 這是**硬閘**（hard gate），不是混合懲罰。路段要麼物理上可行，要麼不可行
- 避免了「部分信用」帶來的單位混合問題（cm² + cdeg²）
- 停車時 heading 不可靠，所以不拒絕任何路段
- GGA-only 模式（sentinel `i16::MIN`）保留原有行為

### 7.3 路段評分與選擇（Filter-then-Rank）

**路段評分：純 Distance 平方**

$$\text{score}(i) = d^2(G, \text{seg}_i)$$

其中 $d^2$ 為 GPS 點到路段的距離平方（詳見 7.1）。Heading **有意義地缺席**於評分函數 — heading 屬於過濾階段，不屬於排名階段。

```rust
/// Distance-squared from GPS point to segment (clamped projection).
/// Heading is intentionally absent — belongs in heading_eligible filter.
pub fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    seg: &RouteNode,
) -> Dist2 {
    distance_to_segment_squared(gps_x, gps_y, seg)
}
```

**路段選擇：Window Search → Grid Search**

採用兩階段搜索，使用 **dual trackers**（雙追蹤器）防止 heading-ineligible 結果掩蓋 eligible-but-farther 結果：

1. **Window Search（窗格搜索）**：
   - 搜索範圍：`[last_idx - 2, last_idx + 10]`
   - 使用 `best_eligible()` 同時追蹤：
     - `best_eligible_*`：通過 heading 過濾的最佳路段
     - `best_any_*`：純 distance 的最佳路段（不管 heading）
   - Early exit：若 eligible 路段在 20m 內 → 直接返回

2. **Grid Search（格網搜索）**（若 window 無 eligible 路段或超出 20m）：
   - 搜索 GPS 位置周圍 3×3 格網
   - **關鍵：Seeding（種子初始化）**
     - 若 `window_eligible = true`：用 window 的 eligible 結果做種子
     - 若 `window_eligible = false`：`best_eligible_dist2 = MAX`（讓第一個 eligible grid 路段勝出）
   - 雙追蹤器持續更新
   - Fallback：若無 eligible 路段 → 回退到 `best_any_idx` 並記錄警告

```rust
/// Scan segment indices, returning (best_eligible, best_any).
fn best_eligible(
    gps_x: DistCm, gps_y: DistCm,
    gps_heading: HeadCdeg, gps_speed: SpeedCms,
    route_data: &RouteData,
    range: impl Iterator<Item = usize>,
) -> (usize, Dist2, bool, usize, Dist2) {
    let mut best_eligible_idx: Option<usize> = None;
    let mut best_eligible_dist2 = Dist2::MAX;
    let mut best_any_idx: Option<usize> = None;
    let mut best_any_dist2 = Dist2::MAX;

    for idx in range {
        if let Some(seg) = route_data.get_node(idx) {
            let d2 = segment_score(gps_x, gps_y, &seg);

            // Update best_any tracker (pure distance)
            if d2 < best_any_dist2 {
                best_any_dist2 = d2;
                best_any_idx = Some(idx);
            }

            // Update best_eligible tracker (heading must pass)
            if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg)
                && d2 < best_eligible_dist2
            {
                best_eligible_dist2 = d2;
                best_eligible_idx = Some(idx);
            }
        }
    }

    let eligible_found = best_eligible_idx.is_some();
    let eligible_idx = best_eligible_idx.unwrap_or(0);
    let any_idx = best_any_idx.unwrap_or(0);

    (eligible_idx, best_eligible_dist2, eligible_found, any_idx, best_any_dist2)
}
```

**不變量（Invariants）：**

| 不變量 | 為什麼重要 |
|--------|------------|
| Early exit 需要 `eligible_found = true` | 防止返回 heading-ineligible 的 window 路段（U-turn bug 修正） |
| `best_eligible_dist2 = MAX` 當 `window_eligible = false` | 防止 heading-ineligible window 結果掩蓋 eligible-but-farther grid 路段 |
| 雙追蹤器從 window 傳承到 grid | Grid 只能改進，不會變差 |
| Grid 設定 `eligible_found = true` 當找到 eligible 路段 | 關鍵：當 `window_eligible = false` 時，grid 可能發現第一個 eligible 結果 |
| Fallback 返回 `best_any_idx` | 當無 eligible 路段時，明確降級到純 distance，而不是靜默錯誤 |

---

## 8. 路線進度投影（模組 ⑤）

### 8.1 GPS 投影至路段（純整數）

選定最佳路段後，計算投影參數 $t$（以 `i64` 避免溢位）：

$$t_\text{num} = (G - P_i) \cdot (P_{i+1} - P_i) \qquad \text{（i64 dot product）}$$

截斷至 $[0,\, \text{len2}]$ 後，計算 route progress（cm）：

$$z_\text{cm} = D[i]_\text{cm} + \frac{t_\text{num} \cdot (\text{seg\_len\_mm} / 10)}{\text{len2}_\text{cm2}}$$

其中 $\text{len2}_\text{cm2} = (\text{seg\_len\_mm} / 10)^2$ 為 runtime 計算。此計算僅需整數乘除法，**不需 `sqrt` 或浮點運算**。

### 8.2 狀態空間模型

定義路線進度 $s(t)$ 為隱藏狀態，GPS 投影 $z(t)$ 為帶雜訊之觀測值（單位均為 cm）：

$$s(t+1) = s(t) + v(t) \cdot 1\ \text{s} \qquad \text{（運動模型）}$$

$$z(t) = s(t) + \varepsilon(t) \qquad \text{（觀測模型，}\sigma_\varepsilon \approx 2000\text{–}3000\ \text{cm）}$$

### 8.3 單調性約束

$$\text{if } z(t) - \hat{s}(t-1) < -5000\ \text{cm} \;\Rightarrow\; \text{reject GPS sample（逆向跳點）}$$

實作使用 -5000 cm (-50 m) 作為實用平衡：
- 容納市區峽谷條件下的 GPS 雜訊
- 捕捉真正的異常（路線反轉、GPS 故障）

此約束是系統穩定性的最強約束之一，以一次 `i32` 減法即可實現。

---

## 9. 速度約束過濾（模組 ⑥）

### 9.1 物理可行距離

在兩次 GPS 更新之間（$\Delta t = 1\ \text{s}$）：

$$D_\text{max} = V_\text{max} \cdot \Delta t + \sigma_\text{gps} = 1667 \times 1 + 2000 = 3667\ \text{cm}\ (\approx 37\ \text{m})$$

其中 $V_\text{max} = 1667\ \text{cm/s}$（60 km/h），$\sigma_\text{gps} = 2000\ \text{cm}$（20 m GPS 裕度）。

### 9.2 跳點拒絕規則

$$|z_\text{new} - \hat{s}_\text{prev}| > D_\text{max} \;\Rightarrow\; \text{reject candidate}$$

以 `i32` 減法配合 `i32::unsigned_abs()` 實現，無浮點。

**拒絕後的行為：** 跳過 Kalman 更新步驟，僅執行 predict step（`ŝ += v̂`），等效於短暫 Dead-Reckoning。`v_gps` 同樣不更新（沿用上一幀 `v̂`）。此機制確保單一跳點不污染 Kalman 狀態。

---

## 10. 一維卡爾曼濾波器（模組 ⑦）

### 10.1 概述

由於路線已線性化為一維，Kalman Filter 設計極為簡潔。狀態向量：

$$\mathbf{X} = \begin{bmatrix} s \\ v \end{bmatrix} \qquad \text{（route progress [cm]，speed [cm/s]）}$$

### 10.2 預測步驟（Prediction Step）

$$\tilde{s}(t+1) = \hat{s}(t) + \hat{v}(t) \cdot 1\ \text{s}$$

$$\tilde{v}(t+1) = \hat{v}(t) \qquad \text{（等速假設）}$$

### 10.3 更新步驟（Update Step）

$$\hat{s} = \tilde{s} + K_s \cdot (z_\text{gps} - \tilde{s})$$

$$\hat{v} = \tilde{v} + K_v \cdot (v_\text{gps} - \tilde{v})$$

### 10.4 Kalman Gain（分子/分母整數）

Kalman gain 直接以分子/分母整數表示，語義最清晰，無需任何 Q 格式轉換：

$$K_s = \frac{51}{256} \approx 0.20, \qquad K_v = \frac{77}{256} \approx 0.30$$

```rust
pub struct KalmanState {
    pub s_cm: i32,   // route progress in cm
    pub v_cms: i32,  // speed in cm/s (always ≥ 0)
}

impl KalmanState {
    /// Cold-start: initialise from first valid GPS projection.
    /// Call once after warm-up period (3 GPS ticks) before entering normal loop.
    pub fn init(z_cm: i32, v_gps_cms: i32) -> Self {
        Self { s_cm: z_cm, v_cms: v_gps_cms.max(0) }
    }

    /// Fixed-point update: Ks = 51/256 ≈ 0.20, Kv = 77/256 ≈ 0.30
    pub fn update(&mut self, z_cm: i32, v_gps_cms: i32) {
        let s_pred = self.s_cm + self.v_cms; // dt = 1s
        let v_pred = self.v_cms;
        self.s_cm  = s_pred + (51 * (z_cm - s_pred)) / 256;
        // Clamp v_cms ≥ 0: bus does not reverse along route;
        // negative v_gps from GPS noise would corrupt the next predict step.
        self.v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0);
    }
}
```

### 10.4.1 自適應 Kalman Gain（HDOP 版）

若 GPS 模組透過 NMEA `$GPGSA`／`$GNGSA` 語句提供 HDOP（Horizontal Dilution of Precision），可依 GPS 幾何精度動態調整 $K_s$，以減少城市峽谷中跳點對 Kalman 狀態的污染。

**設計原則：** HDOP 愈大代表衛星幾何較差、觀測雜訊愈高，此時應降低對 GPS 的信任度（減小 $K_s$），轉而更依賴運動模型的預測值。

| HDOP 範圍 | 精度判斷 | $K_s$（分子/256） | 說明 |
|---------|---------|-----------------|------|
| ≤ 2.0 | 優 | 77（≈ 0.30） | 高精度，充分信任 GPS |
| 2.1–3.0 | 良 | 51（≈ 0.20） | 正常城市環境（預設值） |
| 3.1–5.0 | 普 | 26（≈ 0.10） | 較差，大幅降低 GPS 比重 |
| > 5.0 | 差 | 13（≈ 0.05） | 接近 Dead-Reckoning，僅微修正 |

```rust
/// HDOP-adaptive Kalman gain selector.
/// hdop_x10: HDOP value scaled ×10 as integer
///   (e.g. HDOP 1.8 → hdop_x10 = 18; HDOP 3.2 → hdop_x10 = 32)
pub fn ks_from_hdop(hdop_x10: u16) -> i32 {
    match hdop_x10 {
        0..=20  => 77,   // HDOP ≤ 2.0 → Ks ≈ 0.30
        21..=30 => 51,   // HDOP ≤ 3.0 → Ks ≈ 0.20 (default)
        31..=50 => 26,   // HDOP ≤ 5.0 → Ks ≈ 0.10
        _       => 13,   // HDOP  > 5.0 → Ks ≈ 0.05
    }
}

impl KalmanState {
    /// Adaptive update: Ks varies with GPS quality reported via HDOP.
    /// If HDOP is unavailable, pass hdop_x10 = 25 to fall back to default Ks.
    pub fn update_adaptive(&mut self, z_cm: i32, v_gps_cms: i32, hdop_x10: u16) {
        let ks = ks_from_hdop(hdop_x10);
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm  = s_pred + (ks * (z_cm - s_pred)) / 256;
        self.v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0);
    }
}
```

> **HDOP 不可用時的降級策略：** 若 GPS 模組不輸出 HDOP（例如僅提供基本 `$GPRMC`），直接使用固定增益 `Ks = 51/256` 作為保守預設值，行為與原版 `update()` 完全一致。HDOP 自適應為可選強化，不影響基礎架構。

### 10.5 進階選項：2×2 協方差矩陣

若需進一步降低漂移（理論上再降 20–30%），可引入完整 2×2 協方差矩陣。協方差矩陣本身使用 `f32` 實作（狀態 `s`/`v` 仍維持 `i32` cm）：

$$\mathbf{P}_{t+1} = \mathbf{F} \mathbf{P}_t \mathbf{F}^\top + \mathbf{Q}, \qquad \mathbf{F} = \begin{bmatrix}1 & 1\\ 0 & 1\end{bmatrix}$$

$$\mathbf{K}_t = \mathbf{P}_t \mathbf{H}^\top (\mathbf{H} \mathbf{P}_t \mathbf{H}^\top + \mathbf{R})^{-1}$$

```rust
/// Advanced: full 2×2 covariance Kalman (state stays i32, covariance in f32)
pub struct KalmanFull {
    pub s_cm: i32,
    pub v_cms: i32,
    pub p: [[f32; 2]; 2],  // covariance matrix
}
```

過程雜訊建議：$q_s \approx 100\ \text{cm}^2$，$q_v \approx 25\ \text{(cm/s)}^2$（對應公車加速特性）。協方差矩陣僅 4 個 `f32`（16 bytes），每次更新約 12 次浮點運算，1 Hz 更新率下開銷可接受，**推薦作為後期最佳化選項**。

### 10.6 雜訊參數建議

| 雜訊類型 | 符號 | 建議值 |
|----------|------|--------|
| GPS progress 觀測雜訊 | $\sigma_z$ | 2000–3000 cm |
| GPS speed 觀測雜訊 | $\sigma_v$ | 300 cm/s |
| 過程雜訊（加速度） | $\sigma_a$ | 100 cm/s² |

濾波效果：GPS progress 雜訊 ±3000 cm → Kalman 輸出 ±1000 cm。

---

## 11. 航位推算補償（模組 ⑧）

### 11.1 GPS 斷訊估計

當 GPS 訊號無效時，系統切換至 Dead-Reckoning（DR）模式：

$$\hat{s}_\text{DR}(t) = \hat{s}(t_\text{last}) + \hat{v}_\text{filtered} \cdot (t - t_\text{last})$$

其中 $\hat{v}_\text{filtered}$ 為 EMA 平滑後之速度（以整數 3/10 近似 $\alpha = 0.3$）：

$$\hat{v}_\text{filtered}(t) = \hat{v}_\text{filtered}(t-1) + \frac{3 \cdot (v_\text{gps}(t) - \hat{v}_\text{filtered}(t-1))}{10}$$

### 11.2 DR 時限與範圍限制

DR 最大持續時間：$T_\text{DR,max} = 10\ \text{s}$，對應最大估計誤差 ≈ 150 m。超過此時限，系統進入 `GPS_LOST` 狀態。

### 11.3 GPS 恢復重同步

GPS 恢復後，第一筆資料先通過 Module ⑥ 速度約束過濾（跳點拒絕），驗證通過後才執行 soft correction：

$$\text{若 } |z_\text{gps} - \hat{s}_\text{DR}| > D_\text{max} \;\Rightarrow\; \text{捨棄，繼續 DR（等待下一筆）}$$

$$\text{否則：}\hat{s}_\text{resync} = \hat{s}_\text{DR} + \frac{2 \cdot (z_\text{gps} - \hat{s}_\text{DR})}{10}$$

> GPS 剛恢復的第一筆資料品質最差（HDOP 可能偏高、座標偏移），若直接用於修正 DR 狀態可能引入錯誤。先過速度約束可拒絕明顯跳點；若同時有 HDOP 資訊，建議搭配 10.4.1 的自適應增益進一步降低信任度。

### 11.4 Flash 狀態持久化

設備重啟時從 Flash 讀取上次儲存之狀態，減少冷啟動定位延遲：

```rust
#[derive(Default)]
pub struct PersistedState {
    pub last_progress_cm: i32,
    pub last_stop_index: u8,
    pub checksum: u32, // CRC32 integrity check
}
```

重啟後若 `|current_est - last_progress_cm| > 50000 cm`（500 m），才觸發完整 Stop Index Recovery；否則直接使用 `last_stop_index` 繼續運行，減少誤觸發頻率。

---

## 12. 站點廊道過濾（模組 ⑨）

### 12.1 設計原理

每個停靠站不應被視為點（point），而應視為沿路線方向延伸的廊道區域（corridor）。只有當車輛路線進度 $\hat{s}$ 落入對應廊道範圍時，才啟動該站點之到站檢測。廊道邊界在**離線預處理階段完整計算**並存入 `Stop` 結構，runtime 僅需兩次整數比較。

**v8.4 新增：廊道入口同時作為語音播報觸發點**，不需要額外的距離計算或 Flash 欄位。

### 12.2 廊道定義

$$\text{corridor\_start} = s_i - L_\text{pre} \qquad (L_\text{pre} = 8000\ \text{cm})$$

$$\text{corridor\_end} = s_i + L_\text{post} \qquad (L_\text{post} = 4000\ \text{cm})$$

**廊道非對稱設計的理由：** 前置寬度（80 m）顯著大於後置寬度（40 m），原因在於：

- **提早觸發偵測：** 公車進站前通常已開始減速（距站點約 50–80 m 處），需提早啟動廊道偵測以累積足夠的停留時間特徵（特徵 F₄，$T_\text{ref} = 10\ \text{s}$）。若前置窗口過窄，車輛可能已靠站但 F₄ 尚未達門檻，導致漏報。
- **快速確認離站：** 後置寬度縮短至 40 m，使廊道在公車離站後盡快失效，減少「已離站但仍在廊道內」的 False Positive 時間窗口。
- **緩衝相鄰站點：** 後置短可降低與下一站廊道重疊的機率，搭配 $\delta_\text{sep} = 20\ \text{m}$ 截斷規則確保任兩站廊道之間至少存在 20 m 的不活躍區間。

### 12.3 語音播報觸發

廊道入口（`corridor_start_cm`）距站點 80 m，在市區公車典型速度下自然提供 10–15 秒預告：

| 速度 | 80m 所需時間 |
|------|------------|
| 20 km/h（555 cm/s） | **~14 s** |
| 25 km/h（694 cm/s） | **~12 s** |
| 30 km/h（833 cm/s） | **~10 s** |

公車實際進站前會減速，實際預告時間只會**比上表更長**，對乘客有利。

**觸發條件（全整數，每 tick 評估）：**

| 條件 | 運算式 | 說明 |
|------|--------|------|
| ① 廊道內 FSM 狀態 | `Approaching \| Arriving \| AtStop` | 位置條件已足夠，無需速度門檻 |
| ② 去重保護 | `last_announced_stop != stop_index` | **Per-stop tracking**: 每個站點獨立記錄自身是否已播報。`last_announced_stop` 存儲在 `StopState` 結構中，而非全局變數。當站點播報時，其 `last_announced_stop` 設為自身 index (`self.index`)，防止重複播報。 |


廊道距站點 ≤ 80 m，位置條件本身已排除漂移誤報，速度門檻反而會在塞車慢行場景下阻擋播報，故移除。

```rust
pub struct StopState {
    pub index: u8,
    pub fsm_state: FsmState,
    pub last_announced_stop: u8,  // Per-stop announcement tracking: stores self.index when announced
    // ... 其他既有欄位 (dwell_time_s, last_probability, announced)
}

impl StopState {
    pub fn update(&mut self, s_hat: i32, v_hat: i32, stop_progress: i32, corridor_start: i32, probability: u8) {
        // ...
        // Step 1: FSM 狀態轉移
        self.fsm_state = match self.fsm_state {
            FsmState::Idle => {
                if s_hat >= corridor_start { FsmState::Approaching }
                else { FsmState::Idle }
            }
            FsmState::Approaching => {
                if d_to_stop < 5000 { FsmState::Arriving }
                else if s_hat < corridor_start { FsmState::Idle }
                else { FsmState::Approaching }
            }
            FsmState::Arriving => {
                if probability > 191 { FsmState::AtStop }
                else { FsmState::Arriving }
            }
            FsmState::AtStop => {
                if d_to_stop > 4000 && s_hat > stop_progress { FsmState::Departed }
                else { FsmState::AtStop }
            }
            FsmState::Departed => {
                FsmState::Departed // v8.6: stays in Departed
            }
            FsmState::TripComplete => FsmState::TripComplete,
        };
    }

    /// v8.4: Check for announcement trigger
    pub fn should_announce(&mut self, s_cm: i32, corridor_start_cm: i32) -> bool {
        if s_cm >= corridor_start_cm && self.last_announced_stop != self.index {
            if matches!(self.fsm_state, FsmState::Approaching | FsmState::Arriving | FsmState::AtStop) {
                self.last_announced_stop = self.index;
                return true;
            }
        }
        false
    }
}
```

**設計說明：Per-Stop vs Global Tracking**

v8.4+ 使用 **per-stop tracking** 而非全局變數：
- 每個 `StopState` 維護自己的 `last_announced_stop` 欄位
- 當站點播報時，其 `last_announced_stop` 設為 `self.index`
- 檢查時使用 `self.last_announced_stop != self.index` 判斷是否已播報

**Reset/Reactivation 行為：**
- 恢復（recovery）後，已通過的站點設為 `Departed` 且 `last_announced_stop = i`（自身 index）
- 這確保已通過站點不會被重複播報
- 未播報的站點 `last_announced_stop` 保持為 `u8::MAX`（255），允許首次播報

**計算成本：** 1 次 `matches!` + 1 次整數比較 ≈ **< 0.01 ms**。

### 12.4 廊道重疊保護

若相鄰兩站廊道重疊，離線預處理時截斷。截斷基準為 `corridor_end[i]`（而非 `s_i`），確保兩站廊道之間至少存在 $\delta_\text{sep}$ 的不活躍區間：

$$\text{corridor}_{i+1}.\text{start} = \max\!\left(\text{corridor}_{i+1}.\text{start},\;\; s_i + L_\text{post} + \delta_\text{sep}\right), \quad \delta_\text{sep} = 2000\ \text{cm}$$

即：$\text{corridor\_end}[i] + \delta_\text{sep} = s_i + 4000 + 2000 = s_i + 6000\ \text{cm}$。

> **v8.4 勘誤：** 原公式以 $s_i + \delta_\text{sep}$ 為截斷點，但 $\text{corridor\_end}[i] = s_i + L_\text{post} = s_i + 4000\ \text{cm}$，導致兩廊道仍有 20 m 重疊。正確基準應從 `corridor_end[i]` 起算。

### 12.5 近距離站點廊道調整（v8.6 新增）

當相鄰兩站距離 <120m 時，標準的 20m 重疊保護會導致第二站的 pre-corridor 被過度壓縮。**v8.6 在預處理階段調整廊道分配，確保檢測可靠性。**

#### 12.5.1 問題：壓縮的 pre-corridor

**tpF805 Stop #2 → #3（79m apart）：**

```
標準配置（80m pre + 40m post）+ 20m 重疊保護：
  Stop #3 pre-corridor = 1,442 cm（原應 8,000 cm）

檢測失敗鏈：
  公車進入廊道過晚 → dwell_time_s ≈ 1s → p₄ ≈ 25
  概率 = 185 < 閾值 191 → 漏報 ❌
```

#### 12.5.2 解決方案：動態廊道分配

**觸發條件：** `d < 12,000 cm` (120m)

**廊道重劃比例：**

| 區域 | 比例 | 說明 |
|------|------|------|
| Pre-corridor | 55% | 從站點向後，確保及早進入 |
| Gap | 10% | 兩廊道間緩衝 |
| Post-corridor | 35% | 從站點向前，縮短後置 |

**tpF805 結果（79m → 43.6m pre-corridor）：**

```
Stop #2: corridor_end = 130,465 cm
Stop #3: corridor_start = 131,258 cm (gap: 793 cm)
改善: 1,442 cm → 4,363 cm (3×)
```

**設計考量：**
- **最小距離保護**：d < 2,000 cm 時跳過調整，避免退化廊道
- **與重疊保護互補**：於 `project_stops_validated()` 之後執行，20m gap 仍套用
- **O(n) 預處理成本**：線性掃描，僅執行一次

### 12.6 廊道過濾效果

| 方法 | 誤判率（錯站率） |
|------|-----------------|
| 純距離閾值 < 50 m | ~8% |
| Stop Corridor Filter | < 0.5% |

---

## 13. 到站概率模型（模組 ⑩）

### 13.1 設計框架

到站判定融合四個觀測特徵，各自計算對應似然值後加權合成。全部以整數運算或 LUT 實現，無軟體浮點。

### 13.2 特徵定義與似然函數

所有距離量均使用 **1D 路線進度差**（`|ŝ - s_i|` 或 `|z_gps - s_i|`），符合無 `sqrt` 原則。

#### 特徵 F₁：原始 GPS 距離似然

$$p_1 = \text{gaussian\_lut}(|z_\text{gps} - s_i|,\; \sigma_d = 2750\ \text{cm}) \qquad \text{（u8, 0–255）}$$

使用**未經 Kalman 平滑的原始 GPS 投影** $z_\text{gps}$，反映 GPS 感測器對站點位置的直接觀測。$\sigma_d = 2750$ cm 較寬，容納 GPS 原始雜訊（±30 m）。

#### 特徵 F₂：速度似然

$$p_2 = \text{logistic\_lut}(v_\text{cms},\; v_\text{stop} = 200\ \text{cm/s}) \qquad \text{（u8，128 項 LUT）}$$

#### 特徵 F₃：Kalman 進度差似然

$$p_3 = \text{gaussian\_lut}(|\hat{s} - s_i|,\; \sigma_p = 2000\ \text{cm}) \qquad \text{（u8, 0–255）}$$

使用 **Kalman 平滑後的進度估計** $\hat{s}$，雜訊已壓縮（±10–20 m）。$\sigma_p = 2000$ cm 較窄，提供更精確的位置確認。F₁ 與 F₃ 測量同一物理量（到站距離），但訊號來源不同（原始 GPS vs Kalman 濾波），兩者相關但非冗餘：F₁ 反映當前感測器觀測，F₃ 反映系統整合估計。

#### 特徵 F₄：停留時間

$$p_4 = \min\!\left(\frac{\tau_\text{dwell} \cdot 255}{T_\text{ref}},\;\; 255\right), \quad T_\text{ref} = 10\ \text{s} \qquad \text{（整數線性截飽，無 LUT）}$$

$\tau_\text{dwell}$ 從 FSM 進入 `Approaching`（即 `s_hat >= corridor_start_cm`）時開始計數，單位秒。每個 GPS tick（1 s）遞增 1，離開廊道時重置為 0。

### 13.3 概率融合

各特徵以 `u8` 整數加權，中間值以 `u16` 累加（最大值 32 × 255 = 8160，超出 `u8` 範圍），最終 `>> 5`（÷32）截回 `u8`：

$$P(\text{arrived}) = \frac{13 p_1 + 6 p_2 + 10 p_3 + 3 p_4}{32} \qquad \text{（中間型別 u16，13+6+10+3=32）}$$

```rust
// 中間以 u16 累加，避免 u8 溢位（max = 32×255 = 8160 > 255）
let p_raw: u16 = 13 * p1 as u16 + 6 * p2 as u16
               + 10 * p3 as u16 + 3 * p4 as u16;
let p_arrived: u8 = (p_raw >> 5) as u8;  // ÷32
```

到站觸發條件：

$$P(\text{arrived}) > \theta_\text{arrival} = 191 \qquad \text{（u8 對應 0.75，即 255 × 0.75）}$$

### 13.4 適應性概率權重（v8.6 新增）

**動機：** 對於近距離站點（<120m），dwell time 特徵（F₄）不再是可靠信號。公車可能快速通過而不長停留，導致 p₄ 過低而影響整體概率。

**解決方案：** 當下一站距離 <120m 時，移除 p₄ 權重並重新分配給其他特徵。

**權重調整：**

| 條件 | w₁ (距離) | w₂ (速度) | w₃ (進度) | w₄ (dwell) | 總和 |
|------|-----------|-----------|-----------|------------|------|
| 標準（含末站） | 13 | 6 | 10 | 3 | 32 |
| 近距站點（<120m） | 14 | 7 | 11 | 0 | 32 |

**權重重分配原理：** 原始權重 13+6+10+3=32，移除 w₄ 後剩 29，縮放因子 32/29≈1.103 → (14,7,11,0)

**為何需要 `next_stop` 參數？**

Probability 模型需要「**路線順序的下一站**」距離，而非「**當前活躍的下一站**」。當廊道重疊時，`active_indices` 可能包含多個站點，但「下一個活躍站」的概念不清晰。路線順序是固定且穩定的。

### 13.5 與簡單閾值法之比較

| 方法 | False Positive 率 |
|------|------------------|
| `distance < 50 m`（單一閾值） | 15–30% |
| Stop Probability Model（四特徵） | < 5% |

### 13.6 計算成本（Pico 2）

2 次 LUT 查表 + 1 次線性計算 + 1 次加權求和 ≈ **< 0.1 ms**。

### 13.7 自適應權重實作：順序性 next_stop 傳遞（v8.6 新增）

**關鍵設計：** 概率模型支援傳遞「路線順序的下一站」以判斷是否啟用近站權重。

自適應權重函式 `arrival_probability_adaptive` 需要知道「下一站距離」以判斷是否套用近站權重。此距離應以**路線順序**計算，而非依當前活躍站點動態決定：

| 概念 | 定義 | 問題 |
|------|------|------|
| 活躍的下一站 | `active_indices` 中下一個索引 | 廊道重疊時不明確（多站同時活躍） |
| 路線順序的下一站 | `stops[i+1]` | 固定、穩定、無歧義 |

**實作：** 於 `probability.rs` 模組提供自適應介面。整合時可透過預先建立的 `next_stops` 查詢表，將順序性的下一站資訊傳遞給概率模型，從而實現動態權重切換。

---

## 14. 到站狀態機（模組 ⑪）

### 14.1 狀態定義

| 狀態 | 含義 | 轉入條件（全為整數比較） |
|------|------|----------------------|
| `Approaching` | 進入廊道，正在接近站點；播報邏輯於此啟動 | `s_hat >= corridor_start_cm` |
| `Arriving` | 進入站點近距離區域 | `d_to_stop < 5000 cm` |
| `AtStop` | 確認到站 | `d_to_stop < 5000 cm` AND `P > 191` |
| `Departed` | 離站 | `d_to_stop > 4000 cm` AND `ŝ > s_i` |
| `TripComplete` | 末站離站，本趟行程結束 | 路線最終站進入 `Departed` 狀態 |

> `d_to_stop` 定義：`|ŝ - s_i|`（一維路線進度差，cm）。採用 1D 定義以維持無 `sqrt` 原則；在路線已線性化的前提下，1D 進度差等效於沿路線的實際距離。

### 14.2 狀態轉移規則

```
Idle        → Approaching: s_hat >= corridor_start_cm
                           每 tick 評估播報條件（見 12.3）
Approaching → Arriving:   d_to_stop < 5000 cm
Arriving    → AtStop:     d_to_stop < 5000 cm
                          AND P_arrived > 191
AtStop      → Departed:   d_to_stop > 4000 cm  AND  ŝ > s_i_cm
Departed:                 v8.6: 進入此狀態後不再轉移，確保單次報站
```

**關鍵設計：** 
1. **單向轉移：** 狀態轉移為單向，一旦進入 `Departed` 即無法返回舊站，防止 GPS 漂移引起重複報站。
2. **獨立判定：** 每個站點擁有獨立的狀態機。當前活躍站點由「廊道過濾器」（Module ⑨）動態決定，無需手動維護全局站序索引，天然支援跳站與順序復原。
3. **末站處理：** 路線最後一個站點若進入 `Departed` 狀態，系統可判定本趟行程結束（Trip Complete）。

### 14.3 跳站保護（Skip-Stop Guard）

若 GPS 突然跳至較遠站點（跳過 stop $i$，直接指向 stop $i+2$），狀態機要求必須先進入 stop $i$ 的 `Approaching` 才能觸發到站或播報，否則忽略。跳點經 Module ⑥/⑦ 過濾後通常不會通過廊道入口條件，加上去重保護（`announced`），播報不會誤觸發。

### 14.4 單次到站規則（v8.6 新增）

**規則：公車在一個趟次裡只能到站（報站）一次。**

一旦站點觸發 `AtStop` 狀態（`just_arrived = true`），該站在本趟行程中將無法再次觸發到站事件，即使路線有迴圈經過同一站點。

**實作機制：**

```rust
pub struct StopState {
    // ... 其他欄位
    /// 是否已在本趟行程中到站過（v8.6 新增）
    /// 一旦設為 true，整趟行程無法清除
    pub announced: bool,
}
```

1. **`announced` 欄位：**
   - 在 `Arriving → AtStop` 轉移時設為 `true`
   - 整趟行程永不重置

2. **`can_reactivate()` 函數：**
   - v8.6 之前：允許 `Departed` 狀態的站點重新啟動（用於路線迴圈）
   - v8.6 之後：**永遠返回 `false`**，防止重複到站

3. **`reset()` 函數：**
   - v8.6 之前：重置狀態為 `Idle`
   - v8.6 之後：**No-op**，不改變 FSM 狀態

**問題背景：**

tpF805 路線 Stop #33 發生重複到站問題：
- 第一次到站：time 8733, s_cm=2587371, just_arrived=true ✓
- 離站：time 8745, s_cm=2591734, state=Departed ✓
- **重複到站（BUG）：**time 8765, s_cm=2591376, state=Approaching ❌

**根本原因：**

GPS 雜訊導致公車位置「後退」3.58m（2591734 → 2591376），重新進入廊道觸發 `can_reactivate()`。

**解決方案：**

單次到站規則徹底解決此問題：
- 每個站在一趟行程中只能到站一次
- 無論是 GPS 雜訊或路線迴圈，都不會觸發第二次到站
- 符合實際營運需求：同一趟次不需要重複報站

**測試驗證：**

- `test_one_time_announcement_rule`：驗證 `announced` 標記正確設置
- `test_departed_state_prevents_reactivation`：驗證 `can_reactivate()` 永遠返回 false
- `test_reset_is_noop`：驗證 `reset()` 不改變狀態

**影響評估：**

- ✅ 解決 GPS 雜訊導致的重複到站問題
- ✅ 簡化狀態機邏輯（移除複雜的 reactivation 判斷）
- ✅ 符合實際營運需求
- ⚠️ 路線有迴圈時，第二次經過同一站點不會報站（預期行為）

---

## 15. 站序復原演算法（模組 ⑫）

### 15.1 觸發條件

Recovery 機制只在真正需要時觸發，避免常規運行中誤啟動：

| 觸發條件 | 門檻 |
|---------|------|
| GPS 跳點導致進度突變 | $|\Delta\hat{s}| > 20000\ \text{cm}$（200 m） |
| 重啟後進度不一致 | $|\hat{s} - s_\text{last\_saved}| > 50000\ \text{cm}$（**500 m**） |
| 路線進度與站點持續不符 | $|\hat{s} - s_i| > 20000\ \text{cm}$ 持續 5 s |

重啟門檻設為 500 m 的設計意圖：差距在 500 m 以內時直接信任 Flash 中的 `last_stop_index`，避免正常短暫中斷後誤觸發完整掃描。

### 15.2 復原評分

候選集合（全整數）：

$$\mathcal{C} = \{\, i \mid |s_i - \hat{s}| < 20000\ \text{cm} \;\text{ AND }\; i \geq i_\text{min} \,\}$$

其中 $i_\text{min} = \text{last\_index}.saturating\_sub(1)$，以飽和減法避免 `u8` 下溢（`last_index = 0` 時 $i_\text{min} = 0$）。

評分（取最小值）：

$$\text{score}(i) = |s_i - \hat{s}| + 5000 \cdot \max(0,\;\text{last\_index} - i) + \text{vel\_penalty}(i)$$

速度懲罰：若到達候選站點所需速度超過物理上限，懲罰為 `i32::MAX`（直接排除）。

### 15.3 進度保護

$$\text{stop\_index} \leq \text{index\_of}(\hat{s} + 5000\ \text{cm})$$

### 15.4 穩定性效果

| 場景 | 無 Recovery | 有 Recovery |
|------|------------|------------|
| 長時間運行站序穩定性 | 逐漸偏移 | > 99% |
| 重啟後恢復（差距 < 500 m） | — | 直接信任 Flash，< 0.1 ms |
| 重啟後恢復（差距 > 500 m） | 從頭開始 | 完整掃描 < 0.5 ms |
| GPS 跳點後站序 | 隨機偏移 | 自動校正 |

---

## 16. 脫離路線檢測與恢復（模組 ⑬，v8.9 新增）

### 16.1 概述

公車在實際營運中可能因各種原因脫離預定路線（如：臨時改道、繞路、GPS 飄移導致誤匹配等）。本系統引入脫離路線檢測機制，在偵測到異常時立即凍結位置估計，並在 GPS 回到路線上時執行「重啟」恢復策略。

**設計目標：**
- **快速偵測：** 5 秒內確認脫離路線狀態
- **位置凍結：** 避免錯誤的位置更新累積
- **抑制錯誤播報：** 脫離期間不觸發到站事件
- **智能恢復：** GPS 回到路線時自動跳至正確站點

### 16.2 脫離路線偵測（Off-Route Detection）

系統使用遲滯計數器（hysteresis counter）來避免 GPS 雜訊導致的誤觸發：

**觸發條件（進入 off-route 狀態）：**
$$\text{若 } d^2_\text{match} > \theta^2_\text{off-route} \text{ 連續 } N_\text{confirm} = 5 \text{ ticks}$$

其中：
- $d^2_\text{match}$：地圖匹配的最小距離平方（cm²）
- $\theta_\text{off-route} = 5,000\text{ cm}$（50 m，距離閾值）
- $N_\text{confirm} = 5$：確認計數（避免 GPS 雜訊誤觸發）

**清除條件（離開 off-route 狀態）：**
$$\text{若 } d^2_\text{match} \le \theta^2_\text{off-route} \text{ 連續 } N_\text{clear} = 2 \text{ ticks}$$

使用較短的清除計數（2 ticks）以快速回應 GPS 恢復。

### 16.3 位置凍結（Position Freezing）

一旦進入 `off-route` 狀態，系統立即凍結路線進度估計：

```rust
if state.off_route_tick_count >= OFF_ROUTE_CONFIRM_TICKS {
    state.frozen_s_cm = Some(state.s_cm);  // 記錄凍結位置
    state.off_route_tick_count = 0;
    // 返回 OffRoute 狀態，trace 輸出包含 frozen_s_cm
}
```

**凍結期間行為：**
- $\hat{s}$ 維持在 `frozen_s_cm` 不變
- 到站檢測 FSM 停止更新（不觸發新到站事件）
- 語音播報暫停（避免錯誤通知）
- Trace 輸出 `off_route: true` 標記

### 16.4 路線重入恢復（Re-entry Snap Recovery）

當 GPS 從脫離路線狀態恢復時，系統執行「即時定位」策略：直接跳至 GPS 投影位置，重啟站點檢測。

**觸發條件：**
1. 位置處於確認的脫離路線狀態（`off_route_suspect_ticks >= 5`）
2. 連續 2 次 GPS 地圖匹配品質良好（$d^2_\text{match} \le \theta^2_\text{off-route}$）
3. 凍結狀態被清除（`frozen_s_cm` 變為 `None`）

**恢復演算法：**

當從確認的 `OffRoute` 狀態轉換到 `Normal` 狀態時：

$$\text{Grid Search: } \text{seg}^* = \arg\min_j d^2(\text{GPS}, \text{seg}_j)$$

$$\text{Projection: } z_\text{reentry} = \text{project\_to\_route}(\text{GPS}, \text{seg}^*)$$

$$\text{Snap: } \hat{s} \leftarrow z_\text{reentry},\quad \hat{v} \leftarrow v_\text{GPS}$$

$$\text{Recover: } \text{last\_seg\_idx} \leftarrow \text{seg}^*,\quad \text{in\_recovery} \leftarrow \text{false}$$

**關鍵特性：**
- **即時定位：** 使用全域空間格網（Grid Index）搜尋，不依賴舊的 `last_seg_idx`
- **跳過中間站點：** 若重入位置位於某些站點之後，這些站點將被完全跳過（不會觸發 Approaching/Arriving 狀態）
- **避免錯誤檢測：** 直接跳躍至重入位置，避免漸進式追趕過程中的錯誤站點檢測

**參數設定：**

| 參數 | 值 | 說明 |
|------|-----|------|
| $\theta_\text{off-route}$ | 5,000 cm（50 m） | 地圖匹配距離閾值 |
| $N_\text{confirm}$ | 5 ticks | 進入 off-route 確認計數 |
| $N_\text{clear}$ | 2 ticks | 離開 off-route 清除計數 |
| $V_\text{max}$ | 1,667 cm/s（60 km/h） | 速度約束上限 |

**與 Suspect 狀態的區別：**
- **Suspect → Normal：** 使用漸進式軟同步（soft-resync，2/10 gain）處理小幅 GPS 飄移
- **OffRoute → Normal：** 使用即時定位（immediate snap）處理繞路/長時間脫離

### 16.5 狀態轉移圖

```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    ▼                                     │
┌──────────────┐   d² > θ²   ┌──────────────┐   d² ≤ θ²  │
│   Normal     │ × 5 ticks   │   Off-Route  │ × 2 ticks  │
│              ├────────────►│              ├────────────►│
│ (tracking)   │              │ (frozen)    │              │
└──────────────┘              └──────────────┘              │
      ▲                            │                       │
      │                            │ Immediate snap      │
      │                            │ to GPS projection   │
      │                            ▼                       │
      │                   ┌──────────────┐                │
      └───────────────────│  Re-entry    │────────────────┘
                            │  (snap to    │
                            │   z_reentry) │
                            └──────────────┘

Suspect 狀態（1-4 ticks）：
- Normal → Suspect（d² > θ²，1 tick）
- Suspect → OffRoute（d² > θ²，達 5 ticks）
- Suspect → Normal（d² ≤ θ²，2 ticks 連續）
- Suspect 期間使用 soft-resync 漸進恢復
```

### 16.6 Trace 輸出格式

脫離路線狀態在 trace.jsonl 中輸出以下欄位：

**Off-Route 狀態（位置凍結）：**
```jsonl
{
  "time": 80177,
  "lat": 24.99346,
  "lon": 121.29539,
  "s_cm": 80449,
  "off_route": true,
  "status": "off_route"
}
```

**Re-entry（即時定位恢復）：**
```jsonl
{
  "time": 80223,
  "lat": 24.992078,
  "lon": 121.300427,
  "s_cm": 173088,
  "off_route": false,
  "status": "valid",
  "active_stops": [6],
  "stop_states": [{"stop_idx": 6, "fsm_state": "Approaching"}]
}
```

**欄位說明：**
- `off_route: true`：當前處於脫離路線狀態（位置凍結）
- `off_route: false`：已恢復到正常路線追蹤
- `status: "off_route"` | `"dr_outage"` | `"valid"`：狀態類型
- `s_cm`：路線進度（off-route 期間維持不變，re-entry 時跳至投影位置）
- `active_stops`：恢復後檢測到的目標站點索引（從重入位置開始）

### 16.7 測試驗證

**測試案例：** `ty225_short_detour`

**場景描述：**
- 路線：10 個站點（ty225_short），總長約 2.5 km
- 繞路路徑：Stop 2 (idx 1) → 繞路點 (24.99207, 121.30043) → Stop 7 (idx 6)
- 繞路策略：向東南偏離路線，經過繞路點後回到原路線
- 脫離偏移：距離路線約 300 m
- 跳過站點：Stop 2, 3, 4, 5 (idx 2, 3, 4, 5) 不應被播報

**時間線：**
- Stop 0 (idx 0) arrival: time 80014
- Stop 1 (idx 1) arrival: time 80104
- Off-route detection: time 80177 (5 seconds after departure)
- Detour point reached: time ~80200
- Re-entry snap: time 80223 (s_cm: 102606 → 173088)
- Stop 6 (idx 6) arrival: time 80237
- Off-route duration: ~46 seconds

**驗證結果：**

| 檢查項目 | 結果 | 說明 |
|---------|------|------|
| GPS 連續性 | ✅ PASS | 1 秒間隔，359 個 GPS 更新 |
| 脫離路線檢測 | ✅ PASS | 5 秒後觸發（time 80177） |
| 位置凍結 | ✅ PASS | s_cm 維持在 102,606 cm |
| 跳過站點 | ✅ PASS | Stop 2-5 (idx 2-5) 未被宣告 |
| Re-entry snap | ✅ PASS | s_cm 從 102606 跳至 173088 |
| 繞路持續時間 | ✅ PASS | 46 秒（time 80177-80223） |
| 後續站點恢復 | ✅ PASS | Stop 6-9 (idx 6-9) 正常檢測 |

**Trace 關鍵數據：**
```jsonl
{"time":80104,"stop_idx":1,"s_cm":48066,"event_type":"Arrival"}  // Stop 1 arrival
{"time":80177,"off_route":true,"s_cm":102606,"status":"off_route"}  // Off-route detected
{"time":80227,"off_route":true,"s_cm":102606,"status":"dr_outage"} // Last frozen tick
{"time":80223,"s_cm":173088,"off_route":false,"status":"valid"}    // Re-entry snap confirmed
{"time":80228,"s_cm":167429,"off_route":false,"status":"valid"}    // Re-entry (Suspect→Normal)
{"time":80237,"stop_idx":6,"s_cm":178449,"event_type":"Arrival"} // Stop 6 arrival
```

**執行測試：**
```bash
# 執行 detour 場景測試
make run-detour

# 預期 arrivals: Stop 0 (idx 0), Stop 1 (idx 1), Stop 6-9 (idx 6-9)
# Stop 2-5 (idx 2-5) 應被跳過
```

**實現細節：**
- Off-route 檢測使用 50m 距離閾值（$\theta_\text{off-route} = 5000\text{ cm}$）
- 5-tick 遲滯確認機制避免 GPS 雜訊誤觸發
- Re-entry 時使用全域空間格網搜尋（Grid Index）尋找正確路段
- 直接跳至 GPS 投影位置，避免漸進式追趕過程中的錯誤站點檢測

---

## 17. 隱馬可夫模型地圖匹配（HMM Map Matching，進階選項）

### 17.1 概述

對於城市峽谷環境中存在多條平行道路之場景，可引入 Hidden Markov Model 提升路段匹配之準確性。核心公式為：

$$P(S \mid O) \propto P(O \mid S) \cdot P(S_t \mid S_{t-1})$$

其中 $S$ 為隱藏狀態（路線路段），$O$ 為 GPS 觀測位置。

### 17.2 發射概率（Gaussian LUT）

直接使用第 3 章之 `gaussian_lut`：

$$P(O \mid S=i) = \text{gaussian\_lut}(d_\text{cm}(G, \text{seg}_i),\; \sigma = 2000\ \text{cm})$$

### 17.3 轉移概率

| 轉移類型 | 概率（u8） | 說明 |
|---------|-----------|------|
| 保持同一路段（$i \to i$） | 153（≈ 0.60） | 停站或慢速 |
| 前進一路段（$i \to i+1$） | 89（≈ 0.35） | 正常行駛 |
| 跳過一路段（$i \to i+2$） | 13（≈ 0.05） | 快速通過 |
| 逆退 | 0 | 單調性約束 |

### 17.4 優化候選窗口（Fixed-Window Search）

為了平衡計算量與魯棒性，Map Matching 採用基於上一幀索引的**局部視窗搜尋**：

搜尋範圍：`[last_idx - 2, last_idx + 10]`

此範圍涵蓋了公車在 1 秒內可能行駛的最大距離（含 GPS 跳點補償），並允許小幅度的位置回退。若視窗內搜尋失敗（距離評分過低），則回退至基於空間格網（Grid Index）的全域查詢。

**計算成本：** 最多 13 次路段評分 < 0.5 ms。

### 17.5 簡化 Viterbi（純整數）

$$\text{best\_seg} = \arg\max_i \left[\, P(O \mid S=i) \cdot P(S=i \mid S=\text{prev}) \,\right]$$

所有乘法以 `u8` × `u8` → `u16` 實現（結果 >> 8 正規化）。**計算成本：** 最多 7 次乘法 < 0.1 ms。

---

## 18. 離線預處理流程

此流程在 PC/Server 端完成，產物為 `route_data.bin`。核心原則是：**將所有複雜幾何計算移至線下，確保 Runtime 僅需執行整數查表與簡單加減乘除。**

### 17. 離線預處理流程 (v8.4 更新版)

| 步驟 | 操作名稱 | 詳細說明與約束 |
| :--- | :--- | :--- |
| **1** | **原始資料輸入** | 下載 GeoJSON/JSON 格式之原始路線 Polyline（通常節點間距 1–2m）及 `stops.json`。 |
| **2** | **RDP 幾何簡化** | 使用 Douglas-Peucker 演算法，設定 $\varepsilon_{general} = 700\text{ cm}$。針對 **急彎（轉向 > 20°）** 自動調降至 $\varepsilon_{curve} = 250\text{ cm}$，並啟用 **站點錨點保護**（強制保留站點座標 ±30m 內之節點）。 |
| **3** | **段長約束插值（自適應）** | 檢查簡化後路段，若長度 $> 10000\text{ cm}$（100m），則在中間插入補充節點。**自適應優化：** 站點 ±100m 範圍內及急彎處，段長限制為 30m，確保投影平滑度。 |
| **4** | **累積距離線性化** | 以平面近似法計算各節點累積距離 $D[i]$ (i32 cm)。**注意：** 計算時需使用全局平均緯度 $lat\_avg$。 |
| **5** | **預算幾何係數** | 為每個路段計算並儲存 $dx, dy$ (i16 cm), $seg\_len\_mm$ (i64), $heading\_cdeg$ (i16)。依照 v8.7 規範，**移除 len2 欄位** 以節省空間（改為 runtime 計算）。 |
| **6** | **建立空間格網索引** | 根據線性化後的節點建立 $100\text{m} \times 100\text{m}$ 的 Spatial Grid Index。**v8.8 優化：** 使用 bitmask + u16 offsets，空間從 ~16 KB 降至 ~5 KB。這將用於 DP mapper 的候選路段搜尋。 |
| **7** | **DP 站點投影（dp_mapper）** | **(v8.4 核心更新)** 使用 `dp_mapper` crate 進行**全域最佳化**的站點投影：<br><br>**演算法概述：**<br>- 將站點投影問題轉化為**分層 DAG 最短路徑問題**（Viterbi-like）<br>- 每個站點產生 K 個候選投影（預設 K=15），使用空間格網進行 $O(k)$ 路段查詢<br>- DP 前向傳播：排序掃描找出最小成本路徑（滿足進度單調性）<br>- 回溯重建：從最終層最佳狀態回溯，輸出全域最佳路徑<br><br>**Snap-Forward 機制：**<br>- 對於 j > 0 的站點，若候選進度皆小於前一層最大進度，加入 snap-forward 候選<br>- Snap candidate 錨定於 `max_prev_progress_cm` 之後的首個路段，施加巨大懲罰（`SNAP_PENALTY_CM2 = 10^12 cm²`）<br>- DP 只會在無其他有效轉移時才選擇 snap candidate<br><br>**轉移約束：**<br>- 有效轉移條件：`progress[curr] >= progress[prev]`（允許相等，處理相同位置的相鄰站點）<br>- 支援路線迴圈（同一位置多次經過）<br>- 保證輸出進度值嚴格單調遞增<br><br>**複雜度：** $O(M \times K \log K)$，其中 M = 站點數，K = 候選數<br>- 典型路線（M=35, K=15）：< 10 ms<br>- 大型路線（M=100, K=15）：< 30 ms<br><br>**實作模組：** `preprocessor/dp_mapper/`<br>- `grid/`：空間格網索引<br>- `candidate/`：投影與 K-candidate 選取<br>- `pathfinding/`：DP solver 與回溯 |
| **8** | **計算廊道邊界** | 為每個站點計算非對稱廊道：前置 $80\text{ m}$，後置 $40\text{ m}$。若相鄰廊道重疊，執行 **$\delta_{sep} = 20\text{ m}$ 強制截斷**。 |
| **8.5** | **近距站點廊道調整（v8.6 新增）** | 對站距 $<120\text{ m}$ 的站對，重新分配廊道空間：**55% pre + 10% gap + 35% post**。於 `project_stops_validated()` **之後** 執行，作為標準重疊保護的補強。詳見 Section 12.5。 |
| **9** | **數據打包與校驗** | 將 RouteNode (24 bytes/node)、Stops、Grid 打包。計算 **CRC32** 並標記 **VERSION 5**，產出 `route_data.bin`。<br><br>**詳細規格：** 參見 **[spatial_grid_binary_format.md](spatial_grid_binary_format.md)** - 完整的二進制格式佈局、讀寫實作與 XIP 支援。 |

---

### v8.4 更新重點摘要：

1.  **DP Mapper 取代貪心法：**<br>   - 新增 `preprocessor/dp_mapper` crate，實作全域最佳化站點投影<br>   - 使用 Viterbi-like DAG 演算法，保證找到最小總距離的單調映射<br>   - 解決貪心法在路線迴圈、密集站點等場景下的結構性缺陷（可達 5× 更差）

2.  **候選生成（Candidate Generation）：**<br>   - 空間格網查詢：3×3 → 5×5 → 7×7 漸進式擴展<br>   - 投影至路段，計算距離平方與進度值<br>   - 按距離排序，保留 top-K 候選（預設 K=15）<br>   - 去重依據 `(seg_idx, t)` 避免重複候選

3.  **DP 前向傳播（Forward Pass）：**<br>   - 排序掃描演算法：按進度排序候選後，維護 running minimum<br>   - 時間複雜度 $O(M \times K)$，無需嵌套迴圈<br>   - 自動處理所有單調性約束，無需額外邊界檢查

4.  **Snap-Forward 機制：**<br>   - 確保每個非首站至少有一個可達候選<br>   - 施加巨大懲罰（10^12 cm² ≈ 316 km²），僅在無其他選擇時啟用<br>   - 錨定於 `max_prev_progress_cm` 確保可達性

5.  **測試覆蓋：**<br>   - 單元測試：格網、候選生成、DP solver<br>   - 整合測試：直線路線、L 型路線、迴圈路線、實際 ty225 路線<br>   - 邊界案例：相同位置站點、路段邊界、密集站點、 scalability

6.  **依賴關係：**<br>   - `dp_mapper` 僅依賴 `shared` crate，無外部依賴<br>   - 獨立測試與開發，清晰的模組邊界

### v8.3 保留內容（向後相容）：

1.  **V8.3 空間優化：** 嚴格執行 `RouteNode` 結構體瘦身，移除冗餘的線性方程係數，將 Flash 佔用壓低至約 **24 KB**（以 600 節點計算）。
2.  **一致性校驗：** 強調 `lat_avg` 在線性化與站點投影中必須保持絕對一致，並封裝於 binary header 中供 Runtime 直接讀取。

---

## 19. 效能摘要與資源評估

### 18.1 Pico 2 計算成本（無 FPU，整數實作）

| 模組 | 每次 GPS 更新耗時 | 主要操作 |
|------|-----------------|---------|
| Spatial Grid Index | < 0.1 ms | 整數 grid lookup |
| Map Matching（含 heading） | < 0.5 ms | 10 路段 × 整數 $d^2$ + heading filter |
| Segment Projection | < 0.1 ms | 1 次 `i64` 點積 |
| Speed Constraint | < 0.05 ms | 1 次 `i32` 比較 |
| Kalman Filter（1D 整數增益） | < 0.2 ms | 4 次整數乘加 |
| Dead-Reckoning | < 0.1 ms | 1 次整數乘加 |
| Stop Corridor Check | < 0.05 ms | 2 次整數比較 |
| Stop Probability Model | < 0.1 ms | 2 次 LUT + 加權求和 |
| Stop State Machine | < 0.05 ms | FSM match |
| Stop Index Recovery（觸發時） | < 0.5 ms | 50 次整數比較 |
| **合計（正常模式）** | **< 1.5 ms** | **CPU < 8%（@150 MHz）** |

### 18.2 記憶體佔用

| 資料/狀態 | 佔用 |
|---------|------|
| 路線資料（Flash） | ~20 KB (v8.8) |
| LUT（Flash, 編譯期生成） | 384 bytes |
| Kalman State | 8 bytes（SRAM） |
| DR State | 16 bytes（SRAM） |
| Stop State Machine | 50 bytes（SRAM） |
| Persisted State buffer | 12 bytes（SRAM） |
| GPS 緩衝區、速度歷史 | < 256 bytes（SRAM） |
| **SRAM 合計** | **< 1 KB** |

> **v8.2 更新：** Flash 佔用從 ~34 KB (v8.1) 降至 ~24 KB (v8.2/v8.5/v8.6)，節省 ~10 KB（29% reduction）。
> **v8.8 更新：** Grid 優化從 ~16 KB 降至 ~5 KB，節省 ~11 KB。LUTs 改為編譯期生成（384 bytes），不再存於 bin file。

### 18.3 準確率預估

| 方法演進 | 預估到站準確率 |
|---------|--------------|
| 純距離閾值（< 50 m） | ~80% |
| + Monotonic Constraint | ~88% |
| + Heading-Constrained Map Matching | ~93% |
| + Route Progress Model（1D Kalman） | ~96% |
| + Stop Corridor + Probability Model | ~98% |
| **完整 Pipeline（含 DR + Recovery）** | **≥ 97–99%** |

---

## 20. Embedded Rust 實作注意事項

### 20.1 整數型別別名

全專案統一以下別名，防止混用不同單位：

```rust
pub type DistCm    = i32;  // distance in cm
pub type SpeedCms  = i32;  // speed in cm/s
pub type HeadCdeg  = i16;  // heading in 0.01°
pub type Prob8     = u8;   // 0..255，精度 1/256 已足
pub type Dist2Cm2  = i64;  // squared distance in cm²
```

### 20.2 LUT 生成（編譯期常數）

Gaussian 與 Logistic LUT 於編譯期以 `const fn` 生成，確保與演算法參數同步：

```rust
// crates/pico2-firmware/src/lut.rs
const fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let x = i as i32;
        let x2 = x * x;
        // 簡化 exp(-x²/2) 近似
        let val = if x2 < 64 {
            255 - (x2 / 64) * 50
        } else if x2 < 256 {
            200 - ((x2 - 64) / 192) * 100
        } else if x2 < 576 {
            100 - ((x2 - 256) / 320) * 60
        } else {
            40 - ((x2 - 576) / 64) * 10
        };
        lut[i] = if val < 0 { 0 } else { val as u8 };
        i += 1;
    }
    lut
}

pub static GAUSSIAN_LUT: [u8; 256] = build_gaussian_lut();
```

Logistic LUT 類似生成，佔用 384 bytes Flash。

### 20.3 Flash 資料存取（XIP）

```rust
#[link_section = ".rodata"]
static ROUTE_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/route_data.bin"));
```

Pico 2 支援從 Flash 直接執行（XIP），路線資料不需複製至 SRAM。

### 20.4 並發安全

若需在多核心間共用全域站點索引（如 UI 顯示），應使用 `AtomicU32`：

```rust
use core::sync::atomic::{AtomicU32, Ordering};
static GLOBAL_STOP_INDEX: AtomicU32 = AtomicU32::new(0);
```

### 20.5 啟動暖機（v8.9 更新：估計就緒與檢測門控分離）

系統上電後實施兩階段啟動流程，將「估計就緒」與「檢測門控」分離：

#### 估計就緒（Estimation Readiness）
- **目的**：讓 Kalman Filter 收斂至穩定狀態
- **門檻**：**3 個有效 GPS 更新週期**（或 10 個總週期超時安全閥）
- **影響範圍**：
  - 標題過濾器（Heading Filter）啟用：收斂後使用嚴格角度門檻
  - Kalman Filter 參數：收斂後進入正常估計模式

#### 檢測門控（Detection Gating）
- **目的**：確保到站檢測可靠性
- **門檻**：**3 個有效 GPS 更新週期**（或 10 個總週期超時安全閥）
- **影響範圍**：
  - 到站檢測邏輯：門控期間抑制 Arrival Event
  - 獨立於估計就緒狀態：檢測可透過超時路徑啟用

#### 設計考量
- **超時安全閥**：若 GPS 持續被拒絕或品質不佳，10 個總週期後自動啟用（避免永久卡死）
- **GPS 斷訊重置**：GPS 信號完全消失（>10 s）時，所有計數器重置為 0（保守策略）
- **獨立運作**：估計與檢測使用獨立計數器，可根據需求分別調整門檻值

---

## 21. 完整 Pipeline 總結

| 模組 | 輸入（整數單位） | 輸出 | 核心操作 |
|------|----|------|---------|
| Route Linearization | polyline (lat/lon) | $D[i]$、係數（Flash） | 離線計算，全預載 |
| Polyline Simplification | raw polyline | simplified polyline | DP + 曲線/站點保護 |
| Spatial Grid Index | GPS (x/y cm) | candidate segments | 整數 grid lookup |
| Heading-Constrained MM | GPS + `HeadCdeg` | best segment | 整數 $d^2$，heading filter |
| Segment Projection | GPS + best segment | $z_\text{cm}$（`i32`） | `i64` dot product，整數除法 |
| Speed Constraint | $z_\text{cm}$, $\hat{s}_\text{prev}$ | filtered $z$ / reject | 1 次 `i32` 比較 |
| Kalman Filter (1D) | $z_\text{cm}$, $v_\text{gps}$ | $\hat{s}_\text{cm}$, $\hat{v}_\text{cms}$ | 整數乘加（51/256, 77/256） |
| Dead Reckoning | $\hat{s}_\text{last}$, $\hat{v}$, $\Delta t$ | $\hat{s}_\text{DR}$ | `i32` 乘加 |
| Stop Corridor | $\hat{s}_\text{cm}$, stop list | active stop | 2 次 `i32` 比較 |
| Stop Probability | $d$, $v$, $\Delta p$, $\tau$ | $P$（`Prob8`） | LUT + 整數加權 |
| Stop State Machine | $P$, $\hat{s}$, $\hat{v}$ | `ARRIVED` event | FSM 整數比較 |
| Stop Index Recovery | $\hat{s}$, last\_index | corrected index | 整數評分，門檻 500 m |

### 最終系統效能（Pico 2，無 FPU）

| 指標 | 目標值 | 備註 |
|------|--------|------|
| 到站判定準確率 | ≥ 97% | 城市環境，GPS 誤差 ±30 m |
| GPS 斷訊容忍時間 | 10 s | Dead-Reckoning 補償 |
| CPU 使用率 | < 8% | 1 Hz，整數全 pipeline |
| SRAM 佔用（runtime） | < 1 KB | 路線資料存 Flash（XIP） |
| Flash 佔用 | ~20 KB (v8.8) | 路線資料 + LUT（384 bytes, 編譯期生成） |
| 每次 GPS 更新耗時 | < 1.5 ms | 全 pipeline |
| GPS 恢復後同步時間 | < 2 s | soft correction（2/10 加權） |

> **v8.2 優化：** RouteNode 從 52 → 36 bytes，Flash 佔用從 ~34 KB 降至 ~24 KB（節省 29%）。v8.7 进一步优化至 24 bytes。
> **v8.8 優化：** Grid 使用 bitmask + u16 offsets，從 ~16 KB 降至 ~5 KB。LUTs 改為編譯期生成，不再存於 bin file。

---

## 22. 二層架構設計（v9.0 新增）

### 22.1 設計動機

v9.0 版本將嵌入式韌體重構為明確的二層架構，解決以下問題：

1. **關注點混合**：原架構中，GPS 處理、狀態管理、恢復邏輯混雜在同一模組
2. **測試困難**：無法獨立測試估計邏輯（需同時測試狀態機）
3. **觸發不一致**：部分模式轉換使用內部狀態，部分使用估計信號
4. **恢復邏輯隱式**：恢復作為內聯邏輯散佈於各處，難以驗證

**重構目標：**
- 估計層：純函數，相同 GPS → 相同輸出
- 控制層：狀態機，統一觸發源
- 恢復模組：純函數，明確輸入/輸出

---

### 22.2 架構總覽

```
┌─────────────────────────────────────────────────────────────┐
│                    Control Layer                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ SystemState  │  │ SystemMode   │  │   tick()     │     │
│  │              │  │  (Normal/    │  │ orchestrator │     │
│  │ - mode       │  │   OffRoute/  │  │              │     │
│  │ - frozen_s_cm│  │   Recovering)│  │ 1. estimate()│     │
│  │ - last_stop  │  └──────────────┘  │ 2. transitions│     │
│  └──────────────┘                    │ 3. recover()  │     │
│         ▲                             │ 4. detection()│     │
│         │                             └──────────────┘     │
│         │                                    │              │
└─────────┼────────────────────────────────────┼──────────────┘
          │                                    │
          │ EstimationOutput                   │ EstimationInput
          │ (z_gps_cm, s_cm,                   │ (gps, route_data)
          │  v_cms, divergence_d2)             │
          │                                    │
┌─────────┴────────────────────────────────────┴──────────────┐
│                  Estimation Layer                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ KalmanState  │  │   DrState    │  │  estimate()  │     │
│  │              │  │              │  │              │     │
│  │ - s_cm       │  │ - filtered_v │  │ • Map match  │     │
│  │ - v_cms      │  │ - last_gps   │  │ • Kalman     │     │
│  │ - last_seg   │  │ - in_recovery│  │ • DR         │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
          │
          │ RecoveryInput
          │ (s_cm, v_cms, dt, stops, hint, frozen, window)
          │
┌─────────┴──────────────────────────────────────────────────┐
│                  Recovery Module                           │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  recover() : pure function                           │ │
│  │  • Search: hint_idx ± 10 stops (O(20))              │ │
│  │  • Spatial anchor penalty (off-route recovery)      │ │
│  │  • Velocity constraint (no impossible jumps)        │ │
│  └──────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

### 22.3 控制層（Control Layer）

#### 22.3.1 SystemState 結構

```rust
pub struct SystemState<'a> {
    /// Current operational mode
    pub mode: SystemMode,
    /// Last confirmed stop index (for recovery hint)
    pub last_stop_index: u8,
    /// Frozen position during OffRoute/Recovering (None in Normal mode)
    pub frozen_s_cm: Option<DistCm>,
    /// Hysteresis counter for OffRoute → Normal transition
    pub off_route_clear_ticks: u8,
    /// Hysteresis counter for Normal → OffRoute transition
    pub off_route_suspect_ticks: u8,
    /// Timestamp when OffRoute was entered
    pub off_route_since: Option<u64>,
    /// Timestamp when Recovering was entered
    pub recovering_since: Option<u64>,
    /// Recovery failed flag
    pub recovery_failed: bool,
    /// Route data reference (immutable, XIP-friendly)
    pub route_data: &'a RouteData<'a>,
}
```

#### 22.3.2 SystemMode 枚舉

```rust
pub enum SystemMode {
    /// Normal operation: GPS on-route, detection active
    Normal,
    /// Off-route detected: position frozen, detection suppressed
    OffRoute,
    /// Recovery in progress: searching for correct stop
    Recovering,
}
```

#### 22.3.3 統一觸發系統

所有模式轉換僅使用估計信號：

| 轉換 | 觸發條件 | 遲滯 |
|------|----------|------|
| Normal → OffRoute | `divergence_d2 > 25_000_000` (500 m²) | 5 ticks |
| OffRoute → Normal | `divergence_d2 ≤ 25_000_000` | 2 ticks |
| OffRoute → Recovering | `\|z_gps_cm - frozen_s_cm\| > 5_000` (50 m) | Immediate |
| Recovering → Normal | Recovery success or timeout | - |

**關鍵不變量：**
- 每個 tick 最多一次模式轉換
- `frozen_s_cm` 僅在 OffRoute/Recovering 模式存在
- 恢復僅在 Recovering 模式執行
- 偵測僅在 Normal 模式執行

#### 22.3.4 tick() 協調器

```rust
pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut EstimationState) -> Option<ArrivalEvent> {
    // STEP 1: Isolated estimation
    let est = estimate(input, est_state);

    // STEP 2: State machine transitions (unified triggers)
    // ... single transition per tick ...

    // STEP 3: Recovery (ONLY in Recovering mode)
    if self.mode == SystemMode::Recovering {
        // ... attempt recovery ...
    }

    // STEP 4: Detection (ONLY in Normal mode)
    if self.mode == SystemMode::Normal {
        return self.run_detection(&est, gps.timestamp);
    }

    None
}
```

---

### 22.4 估計層（Estimation Layer）

#### 22.4.1 隔離契約

估計層遵守嚴格的隔離契約：

**不存取：**
- `mode`（控制層狀態）
- `last_stop_index`（控制層狀態）
- `frozen_s_cm`（控制層狀態）

**不觸發：**
- 模式轉換
- 恢復邏輯
- 任何控制層行為

#### 22.4.2 EstimationOutput

```rust
pub struct EstimationOutput {
    /// Raw GPS projection onto route (for F1 probability)
    pub z_gps_cm: DistCm,
    /// Kalman-filtered position (primary position in Normal mode)
    pub s_cm: DistCm,
    /// Filtered velocity (cm/s)
    pub v_cms: SpeedCms,
    /// Divergence from route (squared distance from map matching)
    pub divergence_d2: Dist2,
    /// Confidence signal (0-255, higher is better)
    pub confidence: u8,
    /// Whether GPS has valid fix
    pub has_fix: bool,
}
```

#### 22.4.3 確定性保證

相同 GPS 輸入 → 相同 `EstimationOutput`（無副作用）

```
GPS(t) + route_data + EstimationState(t)
    ↓ estimate()
EstimationOutput(t)
```

此保證使估計層可獨立測試、模擬、驗證。

---

### 22.5 恢復模組（Recovery Module）

#### 22.5.1 純函數設計

```rust
pub fn recover(input: RecoveryInput) -> Option<usize>
```

**輸入：**
```rust
pub struct RecoveryInput<'a> {
    pub s_cm: DistCm,           // Current GPS position
    pub v_cms: SpeedCms,        // Current velocity
    pub dt_seconds: u64,        // Time since off-route
    pub stops: StopsSlice<'a>,  // All stops
    pub hint_idx: u8,           // Last confirmed stop
    pub frozen_s_cm: Option<DistCm>,  // Frozen position
    pub search_window: usize,   // ±N stops to search
}
```

**輸出：**
- `Some(idx)`：恢復成功，返回站點索引
- `None`：恢復失敗，繼續搜尋

#### 22.5.2 搜尋策略

**空間錨點懲罰：**
```
score(i) = d_gps_to_stop(i)² + λ * d_frozen_to_stop(i)²
```

- `d_gps_to_stop(i)`：GPS 到站點 i 的距離
- `d_frozen_to_stop(i)`：凍結位置到站點 i 的距離
- `λ`：懲罰權重（防止跳回舊站點）

**速度約束：**
```
v_max = d(stop_i, stop_hint) / dt
if v_cms > v_max * 1.5:
    reject(stop_i)  # Physically impossible
```

#### 22.5.3 逾時回退

30 秒逾時後，使用幾何搜尋：

```rust
fn find_closest_stop_index(s_cm: DistCm) -> u8 {
    // Find stop with minimum |s_cm - stop.progress_cm|
    // No velocity constraint (fallback strategy)
}
```

---

### 22.6 空間契約（Spatial Contract）

#### 22.6.1 雙空間設計

到站概率模型有意使用兩個空間座標：

| 特徵 | 空間 | 變數 | 目的 |
|------|------|------|------|
| F1（距離） | 原始 GPS 空間 | `z_gps_cm` | 測量「GPS 距離站點多近？」 |
| F3（進度） | 濾波路線空間 | `s_cm` | 測量「沿路線走了多遠？」 |

#### 22.6.2 受控混合策略

當 `divergence > 2000 cm` 時，F1 從 `z_gps_cm` 切換至 `s_cm`：

```rust
let (d1_cm, use_fallback) = if divergence > 2000 {
    ((signals.s_cm - stop.progress_cm).abs(), true)
} else {
    ((signals.z_gps_cm - stop.progress_cm).abs(), false)
};
```

**此為受控防禦策略，非歧義行為：**
- 確定性規則：基於 divergence 閾值
- 防止不良地圖匹配拖累概率
- 閾值 (2000 cm = 20 m) 經實驗驗證

#### 22.6.3 文檔契約

`current_position()` 函數定義單一權威位置：

```rust
pub fn current_position(&self, est: &EstimationOutput) -> DistCm {
    match self.mode {
        SystemMode::Normal => est.s_cm,
        SystemMode::OffRoute => self.frozen_s_cm,
        SystemMode::Recovering => est.z_gps_cm,
    }
}
```

**契約：**
- Normal：Kalman 濾波位置（最佳估計）
- OffRoute：凍結位置（等待恢復）
- Recovering：原始 GPS 投影（搜尋模式）

---

### 22.7 測試與驗證

#### 22.7.1 單元測試

**估計層：**
- `test_estimate_first_fix()`：首次 GPS 定位
- `test_estimate_kalman_update()`：Kalman 濾波更新
- `test_estimate_outage()`：GPS 斷訊處理

**恢復模組：**
- `test_recovery_success()`：正常恢復
- `test_recovery_velocity_constraint()`：速度約束拒絕
- `test_recovery_timeout()`：逾時回退

#### 22.7.2 整合測試

**狀態機：**
- `test_normal_to_offroute()`：正常 → 脫離路線
- `test_offroute_to_recovering()`：脫離路線 → 恢復中
- `test_recovering_to_normal()`：恢復中 → 正常
- `test_single_transition_per_tick()`：單一轉換不變量

---

## 23. 測試案例與驗證

### 23.1 正常營運測試（ty225_normal）

**目的：** 驗證系統在正常路線營運情況下的到站檢測準確性。

**測試配置：**
- 路線：ty225（新界區專線小巴路線）
- 站點數量：58 個站點
- GPS 更新頻率：1 Hz
- 測試場景：正常營運，無捷徑或繞路

**執行測試：**
```bash
make run ROUTE_NAME=ty225 SCENARIO=normal
```

**驗證結果：**
- ✅ GPS 連續性：1 秒間隔
- ✅ 到站檢測：56 個到站事件
- ✅ 宣告事件：58 個宣告事件
- ✅ 處理 GPS 更新：2971 個更新

**輸出檔案：**
- `test_data/ty225_normal_arrivals.json` - 到站檢測結果
- `test_data/ty225_normal_trace.jsonl` - 追蹤記錄
- `test_data/ty225_normal_announce.jsonl` - 宣告事件

### 23.2 捷徑測試（ty225_shortcut）

**目的：** 驗證系統在公車捷徑行駛時的到站檢測行為。

**測試配置：**
- 路線：ty225
- 捷徑：從站點 1 到站點 5（直線距離 1473m，方位角 150.2°）
- 捷徑持續時間：191 秒
- 跳過站點：站點 2、3、4（不應被宣告）

**執行測試：**
```bash
make run ROUTE_NAME=ty225 SCENARIO=shortcut
```

**驗證結果：**
- ✅ GPS 連續性：1 秒間隔
- ✅ 捷徑檢測：正確識別捷徑行駛
- ✅ 到站檢測：56 個到站事件
- ✅ 宣告事件：58 個宣告事件
- ✅ 處理 GPS 更新：2870 個更新

**輸出檔案：**
- `test_data/ty225_shortcut_arrivals.json` - 到站檢測結果
- `test_data/ty225_shortcut_trace.jsonl` - 追蹤記錄
- `test_data/ty225_shortcut_announce.jsonl` - 宣告事件

### 23.3 繞路測試（ty225_short_detour）

**位置：** `test_data/ty225_short_detour_*`

**目的：** 驗證脫離路線檢測與恢復行為，以及繞路期間的正確處理。

**測試配置：**
- 路線：ty225_short（10 個站點）
- 繞路路徑：Stop 2 (idx 1) → 繞路點 (24.99183, 121.297665) → Stop 7 (idx 6)
- 繞路策略：先向西偏離路線，再向南行駛至繞路點
- 跳過站點：Stop 3, 4, 5, 6 (idx 2, 3, 4, 5) 不應被宣告
- 脫離偏移：距離路線約 300 m 的南方偏移
- 驗證：檢測觸發（5 秒）、位置凍結、重新獲取、後續站點恢復

**執行測試：**
```bash
# 執行 pipeline
cargo run -p pipeline -- \
  test_data/ty225_short_detour_nmea.txt \
  test_data/ty225_short_normal.bin \
  test_data/ty225_short_detour_arrivals.jsonl \
  --trace test_data/ty225_short_detour_trace.jsonl
```

**驗證檢查項目：**
1. GPS 連續性 - 1 秒間隔
2. 脫離路線檢測 - 繞路開始後 5 秒觸發
3. 位置凍結 - 脫離路線期間 s_cm 維持恆定
4. 跳過站點 - Stop 3-6 (idx 2-5) 未被宣告
5. 重新獲取 - Stop 7 (idx 6) 正確檢測到站
6. 時間 - 142 秒繞路持續時間

**驗證結果：**
- ✅ GPS 連續性：通過（321 個 GPS 更新，1 秒間隔）
- ✅ 脫離路線檢測：通過（time 80103 觸發，142 個 off_route ticks）
- ✅ 位置凍結：通過（s_cm 維持在 57,972 cm）
- ✅ 跳過站點：通過（Stop 3-6 未被宣告）
- ✅ 重新獲取：通過（Stop 7 於 time 80247 正確檢測）
- ✅ 時間：通過（142 秒繞路持續時間）
- ✅ 繞路點：通過（24.99183, 121.297665 於 time 80144 到達）
- ✅ 後續站點：通過（Stop 8, 9 正常檢測）

**輸出檔案：**
- `test_data/ty225_short_detour_arrivals.json` - 到站檢測結果
- `test_data/ty225_short_detour_trace.jsonl` - 追蹤記錄（含 off_route 狀態）
- `test_data/ty225_short_detour_announce.jsonl` - 宣告事件
- `test_data/ty225_short_detour_summary.md` - 驗證報告

---

## 附錄：參數快速參考

| 參數 | 建議值（整數單位） | 說明 |
|------|---------------------|------|
| GPS 更新率 | 1 Hz（$\Delta t = 1\ \text{s}$） | 硬體決定 |
| Polyline 簡化容差 $\varepsilon$ | 600–800 cm | Douglas-Peucker 一般路段 |
| 急彎保護容差 $\varepsilon_\text{curve}$ | 200–300 cm | 轉彎角 > 20° 處降低容差 |
| 最大路段長度（一般） | 10000 cm（100 m） | 自適應分段：一般路段 |
| 最大路段長度（關鍵區） | 3000 cm（30 m） | 自適應分段：站點 ±100m、急彎處 |
| 站點保護範圍 | ±3000 cm | 強制保留節點 |
| Grid Cell 大小 $\Delta g$ | 10000 cm（100 m） | Spatial Index |
| 方向過濾閾值（硬截止） | 9000 cdeg（90°） | 僅正常速度時啟用粗篩 |
| Heading Ramp 速度 $v_\text{ramp}$ | 83 cm/s（3 km/h） | 漸進 heading 權重起點 |
| GPS 雜訊裕度 $\sigma_\text{gps}$ | 2000 cm | 速度約束中使用 |
| 最大車速 $V_\text{max}$ | 1667 cm/s（60 km/h） | 速度約束上限 |
| 最大進度跳變 $D_\text{max}$ | 3667 cm | $= V_\text{max} \cdot 1\text{s} + \sigma_\text{gps}$ |
| Kalman Gain $K_s$（固定） | 51/256（≈ 0.20） | 整數增益，HDOP 不可用時使用 |
| Kalman Gain $K_s$（自適應） | 13–77/256 | 依 HDOP 動態選擇（見 10.4.1） |
| Kalman Gain $K_v$ | 77/256（≈ 0.30） | 整數增益，speed update |
| EMA 係數 $\alpha_v$ | 3/10（≈ 0.30） | 速度平滑，整數近似 |
| DR 最大時限 | 10 s | Dead-Reckoning |
| DR 重同步 GPS 占比 | 2/10（≈ 0.20） | soft correction |
| 廊道前置寬度 $L_\text{pre}$ | 8000 cm（80 m） | Stop Corridor |
| 廊道後置寬度 $L_\text{post}$ | 4000 cm（40 m） | Stop Corridor |
| 廊道最小分隔 $\delta_\text{sep}$ | 2000 cm（20 m） | 相鄰廊道重疊保護，從 `corridor_end[i]` 起算 |
| **近距站點閾值**（v8.6 新增） | **12,000 cm（120 m）** | **觸發廊道調整** |
| **近距站點 Pre 比例**（v8.6 新增） | **55%** | **近距站點的 pre-corridor 佔站距比例** |
| **近距站點 Post 比例**（v8.6 新增） | **35%** | **近距站點的 post-corridor 佔站距比例** |
| **近距站點 Gap 比例**（v8.6 新增） | **10%** | **兩廊道間的緩衝（自動形成）** |
| **近距站點最小距離**（v8.6 新增） | **2,000 cm（20 m）** | **避免產生退化廊道** |
| Distance sigma $\sigma_d$ | 2750 cm（27.5 m） | Gaussian LUT |
| Progress sigma $\sigma_p$ | 2000 cm（20 m） | Gaussian LUT |
| Speed stop threshold $v_\text{stop}$ | 200 cm/s（7.2 km/h） | Logistic LUT |
| Dwell time reference $T_\text{ref}$ | 10 s | 停留時間特徵 |
| 到站概率閾值 $\theta_\text{arrival}$ | 191（u8 = 255 × 0.75） | 觸發到站事件 |
| HMM 候選窗口 $W$ | 速度自適應 + 2 | 最小 2，最大 ~5 |
| Recovery 搜索範圍 | ±20000 cm（200 m） | Stop Index Recovery |
| Recovery 重啟觸發門檻 | 50000 cm（500 m） | 避免誤觸發 |
| 進度保護裕度 $\delta_\text{guard}$ | 5000 cm（50 m） | Recovery Algorithm |
| **脫離路線距離閾值**（v8.9 新增） | **5,000 cm（50 m）** | **Off-Route Detection** |
| **脫離路線確認計數**（v8.9 新增） | **5 ticks** | **進入 off-route 狀態** |
| **脫離路線清除計數**（v8.9 新增） | **2 ticks** | **離開 off-route 狀態** |
| **GPS 跳躍恢復閾值**（v8.9 新增） | **5,000 cm（50 m）** | **GPS Jump Recovery** |
| **估計就緒時間**（v8.9 更新） | **3 s（3 個有效 GPS）或 10 s 超時** | **Kalman 收斂 + 標題過濾器啟用** |
| **檢測門控時間**（v8.9 更新） | **3 s（3 個有效 GPS）或 10 s 超時** | **到站檢測啟用** |

---

## 附錄 B：到站概率模型權重離線調校流程

### B.1 調校目的

Section 13.3 的融合權重 `13:6:10:3`（對應 F₁:F₂:F₃:F₄ = 0.40:0.20:0.30:0.10）為依工程判斷給出的初始值。在真實路線資料上執行離線調校，可進一步降低 False Positive 與 False Negative 率。

### B.2 資料收集

收集至少 **N ≥ 200 次**真實到站／未到站事件，每筆記錄以下四特徵原始值：

```
event_log: [
  { label: 1,  f1: u8, f2: u8, f3: u8, f4: u8 },  // 1 = 真實到站
  { label: 0,  f1: u8, f2: u8, f3: u8, f4: u8 },  // 0 = 未到站（路過）
  ...
]
```

建議資料集涵蓋：正常到站、GPS 訊號良好、城市峽谷（訊號差）、近距離相鄰站、末站等各種場景。

### B.3 調校方法（Grid Search，整數）

搜尋空間設定：各特徵權重 $w_i \in \{1, 2, \ldots, 16\}$，限制 $\sum w_i = 32$（維持分母為 2 的冪）。

```python
# Offline calibration script (Python, runs on PC)
import itertools, json

def eval_weights(w1, w2, w3, w4, events, threshold=191):
    tp = fp = tn = fn = 0
    for e in events:
        p = (w1*e['f1'] + w2*e['f2'] + w3*e['f3'] + w4*e['f4']) // 32
        pred = 1 if p > threshold else 0
        if pred == 1 and e['label'] == 1: tp += 1
        elif pred == 1 and e['label'] == 0: fp += 1
        elif pred == 0 and e['label'] == 1: fn += 1
        else: tn += 1
    precision = tp / (tp + fp + 1e-9)
    recall    = tp / (tp + fn + 1e-9)
    f1_score  = 2 * precision * recall / (precision + recall + 1e-9)
    return f1_score, precision, recall

events = json.load(open('arrival_events.json'))

best_score, best_w = 0, (13, 6, 10, 3)
# Enumerate combinations where sum = 32, each weight 1..16
for w1 in range(1, 17):
    for w2 in range(1, 17):
        for w3 in range(1, 17):
            w4 = 32 - w1 - w2 - w3
            if 1 <= w4 <= 16:
                score, _, _ = eval_weights(w1, w2, w3, w4, events)
                if score > best_score:
                    best_score, best_w = score, (w1, w2, w3, w4)

print(f"Best weights: {best_w}, F1={best_score:.4f}")
```

Grid Search 搜索空間約 **4,000–8,000 組**（毫秒級完成），可直接輸出新的整數權重。

### B.4 調校結果驗證

將調校後的權重代入 Pico 2 韌體時，建議同步執行以下驗證：

| 驗證項目 | 通過標準 |
|---------|---------|
| True Positive Rate（到站召回率） | ≥ 97% |
| False Positive Rate（錯誤到站率） | ≤ 2% |
| 近距離相鄰站（間距 < 120 m）正確率 | ≥ 95% |
| GPS 訊號差時（HDOP > 4）正確率 | ≥ 90% |

### B.5 閾值 $\theta_\text{arrival}$ 聯調

調校權重後，$\theta_\text{arrival} = 191$ 可能需要小幅調整。建議同時以 PR Curve（Precision-Recall Curve）選取最佳閾值：

$$\theta^* = \arg\max_\theta F_1\text{-score}(\theta)$$

閾值搜尋範圍建議：$\theta \in [160, 220]$（對應 0.63–0.86 概率區間），步長 8（對應 u8 精度 ~3%）。

---

## 版本更新記錄

### v8.7（本版本）← v8.6 (2026-03-31)

**RouteNode 結構優化：40→24 bytes（40% Flash 節省）**

本版本進一步優化 `RouteNode` 結構體，實現更緊湊的記憶體佈局並提升精度。

---

#### 優化目標

- **Flash 節省：** 每節點從 40 bytes → 24 bytes（節省 16 bytes）
- **精度提升：** 段長從 cm → mm（10× 精度）
- **600 節點路線：** 節省 4.8 KB Flash（~24 KB → ~22 KB 總體，-20%）

---

#### 結構體變更

| 變更項目 | v8.5/v8.6 | v8.7 | 說明 |
|----------|-----------|------|------|
| `len2_cm2` | `i64` (8 bytes) | **移除** | 改為 runtime 計算 `(seg_len_mm / 10)²` |
| `seg_len_cm` | `i32` (4 bytes) | `seg_len_mm: i32` (4 bytes) | 精度提升 10×（mm instead of cm），使用 i32 足以容納 100km+ 路線 |
| `dx_cm`, `dy_cm` | `i32` (4 bytes each) | `i16` (2 bytes each) | 100m 段長約束，節省 4 bytes |
| **總大小** | **40 bytes** | **24 bytes** | **-40%** |

---

#### 新記憶體佈局

```rust
#[repr(C)]
pub struct RouteNode {
    // ── i32 fields (4-byte aligned) ────────────────────────────────
    pub x_cm: i32,             // X coordinate (cm)
    pub y_cm: i32,             // Y coordinate (cm)
    pub cum_dist_cm: i32,      // Cumulative distance (cm)
    pub seg_len_mm: i32,       // Segment length (mm, 10× precision)
    // ── i16 fields (2-byte aligned) ────────────────────────────────
    pub dx_cm: i16,            // Segment vector X (cm), max ±100m
    pub dy_cm: i16,            // Segment vector Y (cm)
    pub heading_cdeg: i16,     // Heading in 0.01°
    pub _pad: i16,             // Alignment padding
}
// Total: 24 bytes (16 bytes i32 + 8 bytes i16)
```

---

#### Runtime 影響

**地圖匹配（Module ④）：**
- `len2_cm2` 改為 runtime 計算：`(seg.seg_len_mm / 10) * (seg.seg_len_mm / 10)`
- CPU 成本增加 < 0.1 ms（整數乘法在 ARM Cortex-M33 僅 1-2 週期）

**投影進度（Module ⑤）：**
- 使用 `seg_len_cm = seg.seg_len_mm / 10` 進行投影計算
- 精度提升：mm 單位避免 cm 整數捨入誤差累積

---

#### 二進位格式變更

- **VERSION：** 3 → 4
- **不兼容：** 舊版 `route_data.bin` 無法載入
- **遷移：** 使用新版 preprocessor 重新生成所有 `route_data.bin`

---

#### 測試結果

- **328 測試通過**（100% pass rate）
- **所有測試資料**更新至 VERSION 4
- **Visualizer** TypeScript parser 已更新

---

### v8.8 (2026-03-31) - Grid 空間優化

#### 優化目標
進一步降低 Flash 使用量，針對 Grid 索引進行稀疏化優化。

#### 技術實現

**Bitmask 索引：**
- 1 bit per cell，標記該 cell 是否包含路段
- 空單元格不佔用偏移量空間
- 查詢時先檢查 bitmask，若為 0 則直接返回空切片

**u16 偏移量：**
- 原本使用 u32 (4 bytes) per cell
- 優化為 u16 (2 bytes) per non-empty cell
- 最大偏移 65,535 足以覆蓋 20-40KB 的 route_data.bin

**資料對齊：**
- Cell data section 自動對齊到 2-byte 邊界
- 確保 u16 讀取不會觸發 alignment 錯誤

#### 二進位格式變更

- **VERSION：** 4 → 5
- **不兼容：** 舊版 `route_data.bin` 無法載入
- **遷移：** 使用新版 preprocessor 重新生成所有 `route_data.bin`

#### 空間節省

- **Grid 索引：** ~16 KB → ~5 KB（60-70% 壓縮）
- **典型路線（60×60 grid）：** 14.4 KB → ~5 KB
- **細長路線（稀疏 grid）：** 效果更顯著

#### 效能影響

- **Runtime 查詢：** 增加一次 bitmask 檢查 + popcount 計算
- **CPU 成本：** < 0.01 ms（ARM Cortex-M33 上非常快速）
- **Flash 讀取：** 減少不必要的偏移量讀取

#### 測試結果

- **所有單元測試通過**
- **整合測試通過**
- **Grid 查詢功能驗證正確**

---

#### 技術細節

詳見 [Section 5.5](#55-routenode-結構優化v87) 完整說明。

---

### v8.9 (2026-04-19) - 脫離路線檢測與 GPS 跳躍恢復

#### 新增功能

本版本引入脫離路線檢測與恢復機制，解決公車繞路、GPS 飄移等場景下的錯誤到站問題。

**脫離路線檢測（Off-Route Detection）：**
- 5-tick 遲滯確認機制，避免 GPS 雜訊誤觸發
- 距離閾值：500 m（$d^2 > 25 \times 10^6\ \text{cm}^2$）
- 2-tick 快速清除機制
- Trace 輸出 `off_route` 狀態標記

**位置凍結（Position Freezing）：**
- 脫離路線期間凍結 `s_cm` 進度估計
- 抑制錯誤的到站事件播報
- 記錄 `frozen_s_cm` 供恢復使用

**GPS 跳躍恢復（GPS Jump Recovery）：**
- 偵測 GPS 從脫離路線狀態恢復時的大跳躍（>50 m）
- 自動跳至最近的前方站點（「重啟」行為）
- 速度約束檢查確保時間一致性
- 支援繞路後的正確站點重新獲取

#### 演算法細節

**狀態轉移：**
```
Normal → Off-Route（5 ticks，d² > 閾值）
Off-Route → Normal（2 ticks，d² ≤ 閾值）
Off-Route → Re-acquire（GPS 跳躍恢復）
```

**GPS 跳躍恢復條件：**
1. 位置處於凍結狀態（`frozen_s_cm` 存在）
2. GPS 地圖匹配品質良好（$d^2 \le$ 閾值）
3. GPS 跳躍距離超過 50 m
4. 速度約束檢查通過

**恢復策略：**
- 搜尋所有進度大於凍結位置的站點
- 選擇距離 GPS 投影最近且速度約束允許的站點
- 直接跳躍至該站點的 `progress_cm`
- 清除 `frozen_s_cm` 狀態

#### 測試結果

**ty225_short_detour 測試案例：**
- ✅ GPS 連續性：通過（321 個 GPS 更新）
- ✅ 脫離路線檢測：通過（142 個 off_route ticks）
- ✅ 位置凍結：通過（s_cm 維持在 57,972 cm）
- ✅ 跳過站點：通過（Stop 3-6 (idx 2-5) 未被宣告）
- ✅ 重新獲取：通過（Stop 7 (idx 6) 於 time 80247 正確檢測）
- ✅ 繞路持續時間：通過（142 秒）
- ✅ 繞路點到達：通過（(24.99183, 121.297665) 於 time 80144）
- ✅ 後續站點恢復：通過（Stop 8, 9 正常檢測）

#### 參數新增

| 參數 | 值 | 說明 |
|------|-----|------|
| $\theta_\text{off-route}$ | 5,000 cm（50 m） | 地圖匹配距離閾值 |
| $D_\text{jump}$ | 5,000 cm（50 m） | GPS 跳躍恢復閾值 |
| $N_\text{confirm}$ | 5 ticks | 進入 off-route 確認計數 |
| $N_\text{clear}$ | 2 ticks | 離開 off-route 清除計數 |

#### 檔案變更

**測試資料新增：**
- `test_data/ty225_short_detour_nmea.txt` - 繞路 GPS 追蹤資料（321 個更新）
- `test_data/ty225_short_detour_trace.jsonl` - 追蹤記錄（含 off_route 狀態）
- `test_data/ty225_short_detour_arrivals.jsonl` - 到站檢測結果
- `test_data/ty225_short_detour_gt.json` - 預期結果（Ground Truth）
- `test_data/ty225_short_detour_summary.md` - 測試說明文件

**測試配置：**
- 原始路線：`test_data/ty225_short_normal.bin`（與 ty225_short 測試共用）
- 原始路線：`test_data/ty225_short_route.json`
- 站點資料：`test_data/ty225_short_stops.json`

#### 文件更新

- **Section 16：** 新增脫離路線檢測與恢復完整說明
- **Section 23.3：** 更新繞路測試結果（所有檢查通過）
- **參數快速參考：** 新增 off-route 相關參數

---

### v9.0 (2026-04-28) - 二層架構重構：控制/估計分離

#### 架構重構

本版本將嵌入式韌體重構為明確的二層架構，實現關注點分離與單一職責原則。

**二層架構設計：**

**控制層（Control Layer）：**
- 管理系統模式（Normal/OffRoute/Recovering）
- `tick()` 協調器：每個 tick 執行估計 → 狀態轉換 → 恢復 → 偵測
- 模式轉換基於統一觸發（`divergence_d2`、`z_gps_cm`）
- 每個 tick 最多一次模式轉換（防止競爭條件）

**估計層（Estimation Layer）：**
- 隔離的 GPS → 位置管線（純函數）
- Kalman 狀態 + DR 狀態（內部，控制層不可見）
- 輸出 `EstimationOutput` 包含所有位置信號
- 不觸發：恢復、模式變更、任何控制層行為

**恢復模組（Recovery Module）：**
- 純函數 `recover()` 與明確的 `RecoveryInput`
- 搜尋視窗：hint_idx ± 10 站點（O(20) 效能）
- 空間錨點懲罰（off-route 恢復）
- 速度約束防止物理不可能跳躍

#### 關鍵不變量（Invariants）

1. **估計層隔離**：不存取 `mode`、`last_stop_index`、`frozen_s_cm`
2. **統一觸發**：所有模式轉換僅使用估計信號
3. **單一轉換**：每個 tick 最多一次模式轉換
4. **凍結位置一致性**：`frozen_s_cm` 僅在 OffRoute/Recovering 模式存在

#### 空間契約（Spatial Contract）

本版本明確化到站概率模型的雙空間設計：

**F1（距離似然度）- 原始 GPS 空間：**
- 使用 `z_gps_cm`（GPS 投影至路線的原始位置）
- 目的：測量「GPS 距離站點多近？」
- 捕捉 GPS 不確定性

**F3（進度似然度）- 濾波路線空間：**
- 使用 `s_cm`（Kalman 濾波後的路線位置）
- 目的：測量「我們沿路線走了多遠？」
- 平滑 GPS 雜訊以保持一致性

**受控混合策略：**
- 當 divergence > 2000 cm 時，F1 從 `z_gps_cm` 切換至 `s_cm`
- 此為受控防禦策略，處理不良地圖匹配
- 非歧義行為：基於 divergence 閾值的確定性規則

#### 測試結果

**狀態機整合測試：**
- ✅ Normal → OffRoute 轉換（5-tick 遲滯）
- ✅ OffRoute → Recovering 轉換（大位移）
- ✅ Recovering → Normal 轉換（恢復成功）
- ✅ 恢復逾時回退（幾何搜尋）
- ✅ 單一轉換不變量（每 tick 最多一次模式變更）

#### 檔案變更

**新增模組：**
- `crates/pico2-firmware/src/control/` - 控制層（狀態機、轉換邏輯）
- `crates/pico2-firmware/src/estimation/` - 估計層（Kalman + DR）
- `crates/pico2-firmware/src/recovery.rs` - 恢復純函數

**測試新增：**
- `crates/pico2-firmware/tests/state_machine_test.rs` - 狀態機整合測試

#### 文件更新

- **Section 22：** 新增二層架構設計完整說明
- **Section 13.1：** 新增空間契約文檔（F1/F3 雙空間設計）
- **CLAUDE.md：** 新增架構章節說明二層設計

---

### v8.6← v8.5 (2026-03-29)

**修正：重複到站問題與近距離站點檢測**

本版本包含兩項重要修正，皆在 tpF805 路線測試中發現並修正：

---

#### 修正 1：近距離站點檢測修正（Close Stop Detection Fix）

解決當相鄰兩站距離過近時，因廊道前置空間不足導致 dwell time 特徵過短而漏報的問題。

**問題背景：**

- 當相鄰兩站距離 <120m（如 79m）時，標準廊道配置（80m pre + 40m post）加上 20m 重疊保護
- 導致第二站的 pre-corridor 被壓縮至僅剩 14m（原應為 80m）
- 公車進入廊道過晚 → dwell_time_s 過短 → 概率不足 → **漏報**

**解決方案：三層架構**

1. **Tier 1：順序性 next_stop 傳遞**
   - 於 `DetectionState::process_gps_record` 處理活躍站點時，預先提供下一順位站點資訊。
   - 確保概率模型能正確判斷是否需要啟用適應性權重。

2. **Tier 2：廊道預處理（Section 12.5）**
   - 對站距 <120m 的站對，重新分配廊道空間：55% pre + 10% gap + 35% post
   - 確保第二站的 pre-corridor 至少有 40m 以上（原 14m → 43.6m，3× 改善）

3. **Tier 3：適應性概率權重（Section 13.4）**
   - 偵測到下一站 <120m 時，移除 dwell time（p₄）權重。
   - 權重從 (13,6,10,3) 調整為 (14,7,11,0)，總和維持 32。
   - 避免因 dwell time 過短而導致的誤判。

**測試結果：**

- tpF805 路線 Stop #3（距 Stop #2 僅 79m）：從漏報 → 正常檢測 ✓
- 概率從 185（<191）提升至 222（>191）
- 單元測試：7 個新測試（4 廊道 + 3 概率）
- 整合測試：`scripts/verify_close_stop_fix.sh`

**影響評估：**
- ✅ 解決近距離站點檢測問題
- ✅ 標準站距（>120m）行為不變
- ✅ 向後相容，現有路線資料重新生成即可
- ⚠️ 需重新生成所有 `route_data.bin` 文件

**檔案變更：**
- `preprocessor/src/stops.rs`：新增 `preprocess_close_stop_corridors()`
- `crates/pipeline/detection/src/probability.rs`：新增 `arrival_probability_adaptive()`
- `crates/pipeline/src/lib.rs`：主迴圈邏輯調整

---

#### 修正 2：重複到站問題（Duplicate Arrival Fix）

**問題背景：**

- tpF805 路線 Stop #33 發生重複到站
- 第一次到站：time 8733, s_cm=2587371, just_arrived=true ✓
- 離站：time 8745, s_cm=2591734, state=Departed ✓
- **重複到站（BUG）：**time 8765, s_cm=2591376, state=Approaching ❌

**根本原因：**

1. `can_reactivate()` 函數允許 `Departed` 狀態的站點在重新進入廊道時重新啟動
2. GPS 雜訊導致公車位置「後退」3.58m（2591734 → 2591376）
3. 重新進入廊道觸發 `can_reactivate()`，造成重複到站

**解決方案：單次到站規則（One-Time Announcement Rule）**

**規則：公車在一個趟次裡只能到站（報站）一次。**（詳見 Section 14.4）

實作變更：
1. **新增 `announced` 欄位**：在 `Arriving → AtStop` 轉移時設為 `true`，整趟行程永不重置
2. **停用 `can_reactivate()`**：函數永遠返回 `false`，防止站點重新啟動
3. **`reset()` 改為 No-op**：不再重置狀態為 `Idle`，保持 `Departed` 狀態

**測試結果：**

- 修復前：Stop #33 在 time 8765 重新進入 `Approaching` 狀態 ❌
- 修復後：Stop #33 保持 `Departed` 狀態，無重複到站 ✓
- 單元測試：新增 `test_one_time_announcement_rule` 等 12 個狀態機測試
- BDD 測試：更新 `scenario_stop_reactivation` 與 `scenario_one_time_announcement_prevents_reactivation`
- trace_validator：新增單次到站規則驗證，偵測重複到站問題

**檔案變更：**
- `crates/pipeline/detection/src/state_machine.rs`：新增 `announced` 欄位，停用 reactivation
- `arrival_detector/tests/`：更新測試以反映新行為
- `trace_validator/src/`：新增 `state_transitions` 追蹤與重複到站驗證

---

**綜合影響評估（v8.7 整體）：**

- ✅ 解決 GPS 雜訊導致的重複到站問題
- ✅ 解決近距離站點檢測失敗問題
- ✅ 簡化狀態機邏輯（移除複雜的 reactivation 判斷）
- ✅ 符合實際營運需求（一趟次一次報站）
- ✅ 向後相容，二進制格式不變
- ⚠️ 路線有迴圈時，第二次經過同一站點不會報站（預期行為）

---

### v8.5（本版本）← v8.4 (2026-03-23)

**修正：系統性程式碼審查，共 15 項修正**

功能行為與 v8.4 相同，二進制格式更新為 **VERSION: 3**。

**🔴 嚴重（runtime bug）：**

1. **`RouteNode` repr 修正（Section 5.5）**
   - `repr(C, packed)` → `repr(C)`
   - `packed` 在已有手動 `_pad` 欄位的情況下，對 field 取 `&` 是 UB，修正後 size 變更為 40 bytes (VERSION 3)

2. **Module ⑫ `u8` 下溢保護（Section 15.2）**
   - `last_index - 1` → `last_index.saturating_sub(1)`
   - 首站（`last_index = 0`）時原公式下溢至 255，候選集合為空，復原失效

3. **概率融合中間值溢位（Section 13.3）**
   - 中間型別 `u8` → `u16`（最大值 32×255=8160，超出 `u8`）
   - 新增 Rust 程式碼片段明確標示 `u16` 累加

4. **廊道重疊保護基準修正（Section 12.4）**
   - 截斷點 `s_i + δ_sep` → `s_i + L_post + δ_sep`（= `corridor_end[i] + δ_sep`）
   - 原公式導致相鄰廊道仍有 20 m 重疊

5. **末站 index 溢位保護（Section 14.2）**
   - `Departed` 新增邊界檢查，末站轉入 `TripComplete` 狀態
   - 原設計 `current_stop_index++` 在末站 u8 溢位回到 0

**🟠 設計缺陷：**

6. **F₁ 與 F₃ 訊號來源釐清（Section 13.2）**
   - F₁ 使用原始 GPS 投影 `z_gps`（σ=2750 cm，寬）
   - F₃ 使用 Kalman 平滑後 `ŝ`（σ=2000 cm，窄）
   - 兩者測量同一物理量但訊號來源不同，非冗餘，說明各自作用

7. **`d_to_stop` 統一定義為 1D（Section 14.1）**
   - 明確定義 `d_to_stop = |ŝ - s_i|`（路線進度差，cm）
   - 符合無 `sqrt` 原則；消除全文 `d` / `d_to_stop` / `d_cm` 混用

8. **移除 `Approaching` 死條件（Section 14.1）**
   - 刪除 `d < 12000 cm` 入口條件（廊道內永遠成立，無意義）
   - 入口條件簡化為 `s_hat >= corridor_start_cm`

9. **`τ_dwell` 計數起點明確定義（Section 13.2）**
   - 從 FSM 進入 `Approaching` 時開始計數，離開廊道重置為 0

10. **GPS 恢復先過速度約束（Section 11.3）**
    - soft correction 前先執行 Module ⑥ 跳點拒絕
    - 避免 GPS 恢復後第一筆低品質資料直接污染 DR 狀態

11. **Module ⑥ 拒絕後行為明確定義（Section 9.2）**
    - 拒絕後僅執行 Kalman predict step，等效短暫 DR，不更新 `v_cms`

12. **Kalman `v_cms` 限制非負（Section 10.4）**
    - `update()` 及 `update_adaptive()` 末尾加 `.max(0)`
    - GPS 雜訊產生的負速度會讓 predict step 倒推 `ŝ`，觸發連鎖拒絕

**🟡 文件錯誤：**

13. **移除 Section 7.1 過時 `line_a`/`line_b`/`line_c` 描述**
    - v8.2 已移除這些欄位，但 Section 7.1 仍描述其存在，已清除

14. **Section 14.1 播報邏輯範圍與程式碼對齊**
    - 狀態表更新為與 Section 12.3 程式碼一致（Approaching/Arriving/AtStop）

15. **Kalman 冷啟動初始化明確定義（Section 10.4）**
    - 新增 `KalmanState::init(z_cm, v_gps_cms)` 方法
    - 冷啟動直接以第一筆有效 GPS 投影初始化，搭配 Section 19.5 的 3 s 暖機

---

### v8.4 ← v8.3 (2026-03-23)

**新增：模組 ⑨ 廊道入口語音播報觸發**

**設計決策：**  
廊道入口（`corridor_start_cm`，距站點 80 m）在市區公車典型速度（20–30 km/h）下自然提供 10–15 秒預告，不需要額外的觸發距離計算。公車實際進站前會減速，實際預告時間只會更長，對乘客有利。

**變更內容：**

1. **模組 ⑨（Section 12）：新增 12.3 語音播報觸發**
   - 觸發條件：廊道內任意 FSM 狀態（Approaching / Arriving / AtStop）+ 去重
   - 無速度門檻（廊道位置條件已足夠，速度門檻在塞車場景下會阻擋播報）
   - Runtime 新增 **1 byte**：`last_announced_stop: u8`（納入 `StopState`，初始值 `u8::MAX`）
   - FSM 轉移後再做 Announce 檢查，確保復原後同 tick 跳至 Arriving 時不漏報

2. **模組 ⑪ FSM（Section 14）：播報邏輯整合至既有 `Approaching` 狀態，無新狀態、無重置點**

3. **Phase 3 架構圖（Section 2）：新增 ANNOUNCE Event 輸出**

4. **附錄 A：新增** $V_\text{ann}$、$C_\text{confirm}$ 參數

**影響評估：**
- ✅ Stop struct 不變（12 bytes）
- ✅ 預處理器不變
- ✅ Binary format VERSION 不變（VERSION: 2）
- ✅ Module ⑩/⑫ 完全不變
- ✅ 新增 SRAM：**1 byte**（`AnnouncementState`）
- ✅ Runtime 額外開銷：2 次整數比較 + 飽和加法，< 0.01 ms/tick

**向後相容性：**
- ✅ 現有 `route_data.bin` 無需重新生成
- ✅ 所有現有模組行為不變

---

### v8.4 → v8.3 (2026-03-18)

**新功能：DP Mapper 全域最佳化站點投影**

**變更內容：**
1. **新增 `dp_mapper` crate**
   - 路徑：`preprocessor/dp_mapper/`
   - 實作 Viterbi-like DAG 最短路徑演算法
   - 取代原本的貪心法站點投影，確保全域最佳解

2. **DP Mapper 演算法**
   - **候選生成**：每站產生 K 個候選投影（預設 K=15）
     - 空間格網查詢（100m × 100m cells，3×3 → 5×5 → 7×7 擴展）
     - 投影至路段，計算距離平方與進度值
     - 去重（按 `(seg_idx, t)`）並排序選取 top-K
   - **DP 前向傳播**：排序掃描演算法 $O(M \times K)$
     - 維護 running minimum 找出有效轉移
     - 轉移約束：`progress[curr] >= progress[prev]`
   - **回溯重建**：從最終層最小成本狀態回溯輸出
   - **Snap-Forward 機制**：當無有效轉移時的 fallback
     - 錨定於 `max_prev_progress_cm` 之後的首個路段
     - 施加巨大懲罰（10^12 cm²）確保最後選擇

3. **模組結構**
   ```
   preprocessor/dp_mapper/
     ├── src/
     │   ├── lib.rs           (public API: map_stops)
     │   ├── grid/            (空間格網索引)
     │   ├── candidate/       (候選生成與選取)
     │   └── pathfinding/     (DP solver 與回溯)
     └── tests/
         └── integration.rs   (實際路線測試)
   ```

4. **測試覆蓋**
   - 15 個單元測試（格網、候選生成）
   - 6 個 DP solver 測試
   - 7 個整合測試（含 ty225 實際路線）
   - 1 個 doc test

5. **文件更新**
   - Section 17：更新離線預處理流程，說明 DP mapper 演算法
   - 新增「為何 DP 優於貪心法」比較示例
   - 新增複雜度分析與效能目標

**變更原因：**
- **貪心法結構性缺陷**：局部最佳選擇可能導致全域次優（可達 5× 更差）
- **路線迴圈問題**：貪心法無法正確處理路線回溯、密集站點等場景
- **DP 保證最佳性**：DAG 最短路徑演算法確保找到最小總距離的單調映射

**影響評估：**
- ✅ 站點投影品質顯著提升（全域最佳化）
- ✅ 處理複雜路線（迴圈、密集站點）更可靠
- ✅ 時間複雜度 $O(M \times K \log K)$ 可接受（M=35, K=15 < 10 ms）
- ✅ 獨立 crate，清晰模組邊界
- ⚠️ 預處理時間略增（但仍是離線操作，無影響）

**向後相容性：**
- ✅ 輸出格式不變（`Vec<Candidate>` 進度值）
- ✅ 現有 `.bin` 路線檔案仍然有效
- ✅ Runtime 無需修改

---

### v8.3 → v8.2 (2026-03-17)

**優化：移除未使用的 `line_a`/`line_b`/`line_c` 係數**

**變更內容：**
1. **RouteNode 結構體優化**
   - 移除 `line_a`（i32, 4 bytes）、`line_b`（i32, 4 bytes）、`line_c`（i64, 8 bytes）
   - 結構體大小：52 → 36 bytes（每節點節省 16 bytes）
   - 欄位重新排列以維持 8-byte alignment

2. **預處理器更新**
   - `preprocessor/src/linearize.rs`：移除 line_a/b/c 係數計算
   - 減少離線預處理計算量

3. **測試更新**
   - `v8_binary_verification.rs`：移除 line coefficient invariant 測試，改為驗證 len2 = dx² + dy²

4. **二進制格式版本**
   - `VERSION` 從 1 → 2（breaking change）
   - 所有現有 `route_data.bin` 需重新生成

5. **文件更新**
   - Section 5.3：更新係數表，移除 line_a/b/c
   - Section 5.4：更新 RouteNode 結構文件（36 bytes）
   - Section 7.1：更新距離計算方法描述
   - Section 17、18：更新 Flash 佔用估算（~34 KB → ~24 KB）
   - `dev_guide.md`：更新範例程式碼

6. **序列約束站點投影（Sequence-Constrained Stop Projection）**
   - `preprocessor/src/stops/validation.rs`：新增驗證模組
   - 路徑約束格網搜索：每個站點只能匹配 >= 前一站點的路段索引
   - 漸進式視窗擴展：3×3 → 5×5 → 7×7 → 線性回退
   - 單調性驗證：確保進度值按輸入順序嚴格遞增
   - 預處理器版本更新至 v8.3 Pipeline
   - **簡化設計：** 移除自動重試機制（epsilon 降低），驗證失敗直接報告錯誤

**變更原因：**
- Runtime 採用點積投影法，不需要線性距離公式係數
- `line_a`/`line_b`/`line_c` 僅用於測試驗證，從未被 runtime hot path 使用
- 移除可節省 16 bytes/節點，600 節點約節省 **9.6 KB Flash**

**影響評估：**
- ✅ Flash 佔用減少 29%（~34 KB → ~24 KB）
- ✅ 結構體更簡潔，減少混淆
- ⚠️ Binary format breaking change（VERSION: 1 → 2）
- ⚠️ 需重新生成所有 `route_data.bin` 文件
- ✅ Runtime 行為不變（速度、準確率無影響）

**向後相容性：**
- ❌ 不相容：所有 `route_data.bin` 文件需使用新預處理器重新生成
- ✅ Runtime 邏輯完全相容（僅讀取更小的結構體）

---

### v8.2 → v8.1 (2026-03-17)

**文件更正：Section 7.1 距離計算方法**

**變更內容：**
- 更新 Section 7.1 以反映實際實作採用**點積投影法**（Dot Product Projection）而非原描述之線性距離公式 $d^2 = (Ax + By + C)^2/(A^2 + B^2)$
- 說明 `line_a`/`line_b`/`line_c` 係數仍於離線預算並儲存之原因：
  1. 完整性文件（記錄直線方程式）
  2. 調試與驗證工具
  3. 未來擴展性（HMM 地圖匹配等）
  4. 向後相容性

**變更原因：**
- 實際程式碼（`crates/pipeline/gps_processor/src/map_match.rs`）使用點積投影法計算距離
- 點積投影法對線段邊界處理更直觀（投影點 clamp 至路段範圍）
- 與線性距離公式在數學上等價，但實作更清晰

**影響評估：**
- 無功能變更，僅文件更正
- 無需更新資料結構或預處理程式
- Runtime 行為保持不變

---

### v8.1 → v8.0 (2026-03-15)

**到站狀態機（模組 ⑪）更新**

**變更內容：**
1. **移除速度閾值** - 原本要求 `v_cms < 56`（約 2 km/h）才能觸發到站，現已移除此限制
2. **放寬距離閾值** - 從 `d_to_stop < 3000 cm`（30 m）調整為 `d_to_stop < 5000 cm`（50 m）

**新觸發條件：**
```
AtStop: d_to_stop < 5000 cm AND P_arrived > 191
```

**變更原因：**
- 實際測試發現部分公車在站點附近停車時，因 GPS 漂移導致投影位置落於站點後方 30-40m 處
- 即使速度已降至接近 0 km/h，仍因距離超過 30m 而無法觸發到站
- 移除速度閾值並放寬距離至 50m，可容納 ±10-20m 的 GPS 誤差邊際，同時依賴概率模型（P > 191）過濾誤判

**影響評估：**
- **優點**：提升到站檢測率，特別是在城市峽谷或 GPS 訊號較差的環境
- **風險**：可能增加誤判率，但透過概率閾值 191（75%）可有效控制
- **測試結果**：downtown 測試案例從 50%（2/4）提升至 100%（4/4）檢出率

**向後相容性：**
- 現有路線資料（.bin）無需更新
- 概率模型權重與閾值保持不變
- 建議在實際部署後收集真實數據，監控 False Positive 率是否在可接受範圍內