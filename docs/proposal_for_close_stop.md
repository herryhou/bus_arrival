## 推薦方案：三層防禦

---

### Tier 1：修 Bug（必做）

驗證並修正主迴圈確實迭代 `find_active_stops()` 回傳的**所有** stop index：

```rust
// main.rs - 確認這裡是 for loop 而非只取第一個
let active_indices = corridor::find_active_stops(record.s_cm, &stops);
for &stop_idx in &active_indices {          // ← 必須是全部，不能是 [0]
    process_stop(record, stop_idx, &stops, &mut stop_states);
}
```

**效果：** Stop #3 從 time=471 就開始累積 dwell，到 time=483 時 dwell≈6s，p4≈153，機率升至 ≈197 ✓，問題可能就此解決。

---

### Tier 2：Close-Stop Corridor 重劃（Preprocess）

**設計思路：** 與其在 runtime 動態調整 departure 閾值，不如在 route preprocess 階段就將站距過近的站對 corridor 重新切分，runtime 完全不需要感知「這是近距離站對」。

**觸發條件：** 當相鄰兩站距離 `d < 12,000 cm (120m)` 時（即正常 corridor 會產生 overlap 的臨界），對此站對套用以下比例：

```
corridor_start (pre)  = 0.55 × d
corridor_end   (post) = 0.35 × d
separation gap        = 0.10 × d   ← 兩 corridor 之間的緩衝
```

三段比例精確填滿站間距：`0.55 + 0.10 + 0.35 = 1.00 × d` ✓，無 overlap，無 gap 浪費。

**具體數值（Stop #2 / #3，d = 7,932 cm）：**

```
Stop #2  progress = 127,689 cm
  corridor_start = 127,689 - 0.55 × 7,932 = 127,689 - 4,363 = 123,326 cm
  corridor_end   = 127,689 + 0.35 × 7,932 = 127,689 + 2,776 = 130,465 cm

Gap = 0.10 × 7,932 = 793 cm (≈8m)

Stop #3  progress = 135,621 cm
  corridor_start = 135,621 - 0.55 × 7,932 = 135,621 - 4,363 = 131,258 cm
  corridor_end   = 135,621 + 0.35 × 7,932 = 135,621 + 2,776 = 138,397 cm
```

```
Position (cm):  122k    124k    126k    128k    130k    132k    134k    136k    138k    140k
               |       |       |       |       |       |       |       |       |       |
Stop #2:       |---[===STOP2===]---|
Gap (8m):                          |----|
Stop #3:                                |---[===STOP3===]---|
               ↑                   ↑   ↑                   ↑
            123326              130465 131258             138397
```

**重劃後 timeline（關鍵改善）：**

| Time | s_cm | Stop #2 corridor | Stop #3 corridor | Stop #3 dwell |
|------|------|-----------------|-----------------|--------------|
| 465  | 129,889 | AtStop ✓ | — (未進入) | — |
| 467  | 131,028 | **已離開** (>130,465) | Approaching | 1s |
| 469  | 132,323 | — | Approaching | 2s |
| 471  | 133,433 | — | Arriving | 3s |
| 473  | 134,571 | — | Arriving | 4s |
| 475  | 135,883 | — | Arriving | 5s |
| 483  | 140,609 | — | **AtStop?** | 8s |

Time=483 時 dwell=8s，p4 = (8×255)/10 = **204**，機率重算：
```
(13×255 + 6×0 + 10×255 + 3×204) / 32
= (3315 + 0 + 2550 + 612) / 32
= 6477 / 32 = 202 ✓   (threshold=191，margin=+11)
```

**Preprocess 實作：**

```rust
/// 在 route 初始化時呼叫，修改 close stop pair 的 corridor 邊界
pub fn preprocess_close_stop_corridors(stops: &mut [Stop]) {
    const CLOSE_STOP_THRESHOLD_CM: i32 = 12_000; // 120m
    const PRE_RATIO: i32 = 55;   // 0.55 × d
    const POST_RATIO: i32 = 35;  // 0.35 × d
    // gap = 0.10 × d 自然形成，不需額外處理

    for i in 0..stops.len().saturating_sub(1) {
        let d = stops[i + 1].progress_cm - stops[i].progress_cm;
        if d < CLOSE_STOP_THRESHOLD_CM {
            // 縮短 stop[i] 的 post corridor
            stops[i].corridor_end_cm =
                stops[i].progress_cm + d * POST_RATIO / 100;
            // 縮短 stop[i+1] 的 pre corridor
            stops[i + 1].corridor_start_cm =
                stops[i + 1].progress_cm - d * PRE_RATIO / 100;
        }
    }
}
```

**架構優勢：**
- Runtime 路徑零改動，`find_active_stops()` 和 FSM 完全不感知此邏輯
- `corridor_start_cm` / `corridor_end_cm` 已是 `Stop` struct 既有欄位
- 只需在 route flash 前呼叫一次，屬於 build-time cost

---

### Tier 3：Adaptive Probability Weights（保護層）

p4 對近距離站點不是 signal，是 penalty。當次站距離 < 120m（與 Tier 2 觸發條件一致），應直接移除 p4 的權重，並**保持總權重 = 32**（防止 u8 overflow）：

```rust
// probability.rs
let (w1, w2, w3, w4): (u32, u32, u32, u32) = if next_stop_dist_cm < 12_000 {
    (19, 5, 8, 0)   // 移除 p4，重新分配，sum = 32
} else {
    (13, 6, 10, 3)  // 標準，sum = 32
};

let prob = ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8;
```

**數值驗證（Stop #3 at time=483，萬一 Tier 2 仍不足時）：**
```
(19×255 + 5×0 + 8×255 + 0×25) / 32
= (4845 + 0 + 2040 + 0) / 32
= 6885 / 32 = 215 ✓  (margin: +24)
```

> ⚠️ 分析文件的 Solution 3 權重 `(15, 8, 12, 0)` sum=35，會讓最大值超過 255 溢出，需使用上面修正後的版本。

---

### 各 Tier 對比

| | Tier 1 | Tier 2 | Tier 3 |
|---|---|---|---|
| **性質** | Bug fix | Preprocess 重劃 | 機率模型調整 |
| **修改位置** | `main.rs` | Route init | `probability.rs` |
| **Runtime 成本** | 零 | 零 | 零（分支預測） |
| **Overlap 消除** | 否（允許共存） | **是（根本解）** | 否 |
| **必要性** | 必做 | 強烈建議 | 作為最終保護層 |

Tier 2 是真正的根本解——將問題從 runtime 的動態協調移到 preprocess 的靜態重劃，runtime code 完全不需要理解「近距離站對」的概念。