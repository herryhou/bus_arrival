# P1: 🔴 The issue (and it’s conceptual)

## ❗ You now have **two competing recovery systems**

### System A — “Snap re-entry” (in Kalman layer)

```rust
if had_frozen_position && state.frozen_s_cm.is_none() {
    state.s_cm = z_reentry;
    return ProcessResult::Valid { ... };
}
```

### System B — “Stop index recovery” (in detection layer)

```rust
find_stop_index(...)
```

---

## 🚨 Problem

These two systems operate with **different assumptions**:

| System   | Basis                  | Output     |
| -------- | ---------------------- | ---------- |
| Snap     | spatial GPS projection | `s_cm`     |
| Recovery | stop-level inference   | `stop_idx` |

They are **not coordinated**.

---

## 💥 Failure mode (still exists)

### Scenario: long detour, rejoin far ahead

1. Re-entry happens
2. Snap sets:

   ```rust
   s_cm = z_reentry
   ```
3. Detection resumes immediately
4. BUT:

   * `last_known_stop_index` still reflects pre-detour state
   * FSM not yet aligned

→ Then recovery logic may still trigger and:

* override correct snap result
* or misinterpret position

---

## 📌 Key insight

> You should either:

* **Snap and trust spatial position**
  OR
* **Run recovery and infer stop index**

👉 Right now you are doing both.

---

## ✅ Minimal fix (don’t over-engineer)

When snap happens:

```rust
return ProcessResult::Valid { ... };
```

You must ALSO:

### 1. Disable recovery trigger

Something like:

```rust
self.needs_recovery_on_reacquisition = false;
```

(or equivalent in state layer)

---

### 2. Force stop index alignment

Immediately after snap:

```rust
self.last_known_stop_index = find_closest_stop_index(s_cm);
```

You already have helper:

```rust
fn find_closest_stop_index(&self, s_cm: DistCm) -> u8
```

👉 Without this, detection is running with stale topology.

---

### 3. Reset FSM consistently

You already implemented:

```rust
reset_stop_states_after_recovery(...)
```

👉 But I don’t see it being called during snap path.

This is critical.


===

# 修正方案：
根據審查意見把這個修復實現成具體的程式碼變更。整體方向是 **讓 snap 成為「真理」，後續恢復路徑必須在空間和時間上都讓位**。

---

## 1. `kalman.rs` – 讓 `ProcessResult::Valid` 攜帶 `snapped` 信號

```rust
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,
        v_cms: SpeedCms,
        seg_idx: usize,
        snapped: bool,   // 新增：此 Valid 是否來自 off-route 重入的立即 snap
    },
    // 其他 variant 不變
    // ...
}
```

在 snap 發生的地方顯式設爲 `true`（原程式碼中 `OffRouteStatus::Normal` 且 `had_frozen_position` 的分支）：

```rust
if had_frozen_position && state.frozen_s_cm.is_none() {
    // 重入 snap
    let (new_seg_idx, _) = crate::map_match::find_best_segment_grid_only(...);
    let z_reentry = crate::map_match::project_to_route(...);
    state.s_cm = z_reentry;
    // ... 其他狀態更新 ...

    let signals = PositionSignals { z_gps_cm: z_reentry, s_cm: state.s_cm };
    return ProcessResult::Valid {
        signals,
        v_cms: state.v_cms,
        seg_idx: new_seg_idx,
        snapped: true,      // <-- 關鍵
    };
}
```

其他地方返回 `Valid` 都加上 `snapped: false`（可以寫一個輔助建構子或直接補全）。

---

## 2. `state.rs` – 處理 snap 後的「空間主導」對齊

在 `process_gps` 匹配 `ProcessResult::Valid` 並做完基本的 `first_fix`、`warmup` 檢查後，**立刻**插入 snap 專用邏輯。  
同時我們新增一個 **debounce 計數器** `just_snapped_ticks`（放在 `State` 結構中）來避免後續幾秒的 recovery 干擾。

### 2.1 新增狀態欄位

```rust
pub struct State<'a> {
    // ... 原有欄位 ...
    pub just_snapped_ticks: u8,   // snap 後的靜默期（單位 tick）
}
```

初始化時設爲 0。

### 2.2 核心處理流程（在 `Valid` 分支內）

```rust
ProcessResult::Valid { signals, v_cms, seg_idx, snapped } => {
    let PositionSignals { z_gps_cm: _, s_cm } = signals;
    let gps_status = GpsStatus::Valid;

    // --- 原本的 first_fix 與 warmup 處理（略） ---

    // 若本 tick 仍在 snap 冷卻期，強制跳過 recovery
    let in_snap_cooldown = self.just_snapped_ticks > 0;
    if in_snap_cooldown {
        self.just_snapped_ticks = self.just_snapped_ticks.saturating_sub(1);
    }

    // ==== SNAP 主導對齊 ====
    if snapped {
        // 1. 前向搜尋最接近的站點（利用已知方向）
        let new_idx = self.find_forward_closest_stop_index(s_cm, self.last_known_stop_index);
        self.last_known_stop_index = new_idx;

        // 2. 基於幾何重置所有站點的 FSM（避免誤標 Approaching/Departed）
        self.reset_stop_states_after_snap(new_idx, s_cm);

        // 3. 清除所有可能觸發後續 recovery 的狀態
        self.needs_recovery_on_reacquisition = false;
        self.kalman.freeze_ctx = None;           // 不再需要凍結上下文
        self.last_valid_s_cm = s_cm;             // 重置基準，避免跳躍偵測誤觸發
        self.just_snapped_ticks = 2;             // 啟動 2 秒冷卻期

        // 直接進入檢測階段，不執行任何 recovery 路徑
        // (後面的 if !snapped && ... 條件會自然跳過)
    }

    // --- 原本的 GPS jump 偵測（H1）---
    if !snapped && !in_snap_cooldown && !self.first_fix && should_trigger_recovery(s_cm, prev_s_cm) {
        // ... 原有邏輯 ...
    }

    // --- 原本的 re-acquisition recovery ---
    if !snapped && !in_snap_cooldown && self.needs_recovery_on_reacquisition {
        // ... 原有邏輯 ...
    }

    // 之後的常規檢測（corridor filter、probability、FSM 更新）保持不變
}
```

### 2.3 前向最近站點搜尋

避免在邊界處誤選後方的站點：

```rust
/// 在 [last_known_stop_index, 路線末端] 範圍內找距離 s_cm 最近的站點
fn find_forward_closest_stop_index(&self, s_cm: DistCm, last_idx: u8) -> u8 {
    let mut best_idx = last_idx;
    let mut best_dist = i32::MAX;

    for i in last_idx as usize .. self.route_data.stop_count {
        if let Some(stop) = self.route_data.get_stop(i) {
            let d = (s_cm - stop.progress_cm).abs();
            if d < best_dist {
                best_dist = d;
                best_idx = i as u8;
            }
        }
    }
    best_idx
}
```

（若擔心 `last_idx` 剛好就是最佳但可能已過站，此搜尋仍會選它；但後面的 FSM 重置會按幾何處理）

### 2.4 基於幾何的 FSM 重置（`reset_stop_states_after_snap`）

取代原本的 `reset_stop_states_after_recovery`，我們直接寫一個專用版：

```rust
fn reset_stop_states_after_snap(&mut self, current_idx: u8, s_cm: DistCm) {
    for i in 0..self.stop_states.len() {
        let st = &mut self.stop_states[i];
        let stop = match self.route_data.get_stop(i) {
            Some(s) => s,
            None => continue,
        };

        if i < current_idx as usize {
            // 已經過去的站點標爲 Departed
            st.fsm_state = FsmState::Departed;
            st.announced = true;                     // 防止再報
            st.last_announced_stop = i as u8;
        } else if i == current_idx as usize {
            // 當前站點：基於實際幾何關係設定狀態
            let dist_to_stop = (s_cm - stop.progress_cm).abs();
            if dist_to_stop < 5000 {
                // 已在站點附近 -> 直接設爲 AtStop（避免重複抵達邏輯可後續處理，但至少不是 Approaching）
                st.fsm_state = FsmState::AtStop;
                st.announced = true;   // 標記已報站，避免後續 trigger
            } else if s_cm > stop.progress_cm + 4000 {
                // 已明顯駛離
                st.fsm_state = FsmState::Departed;
                st.announced = true;
            } else {
                // 仍在接近中
                st.fsm_state = FsmState::Approaching;
                // 不清除 announced，讓後續檢測決定是否觸發 arrival
            }
            st.last_announced_stop = i as u8;
        } else {
            // 未來的站點保持 Idle
            st.fsm_state = FsmState::Idle;
            st.announced = false;
            st.last_announced_stop = u8::MAX;
        }
        st.dwell_time_s = 0;
        st.previous_distance_cm = None;
    }
}
```

這樣做保證了 snap 後 FSM 的狀態完全由物理位置決定，而不僅僅是索引推算。

### 2.5 回收冷卻期與清理

- `just_snapped_ticks` 在每 tick 開始時遞減，並作爲 guard 阻擋 recovery 邏輯。
- `freeze_ctx` 在 snap 時清除，避免後續 recovery 評分被舊的凍結上下文影響。
- `last_valid_s_cm` 重置爲當前 `s_cm`，防止下一次跳躍偵測誤判。

---

## 3. 需要注意的細節

- **所有返回 `Valid` 的路徑**（包括 first fix、常規 Kalman 更新等）都要補上 `snapped: false`。  
- `in_snap_cooldown` 與 `snapped` 的 guard 需同時應用於 `should_trigger_recovery` 和 `needs_recovery_on_reacquisition` 兩個區塊。  
- `reset_stop_states_after_snap` 中對於當前站點的處理必須考慮可能已經觸發過 arrival 的情況，因此直接設爲 `AtStop` 並標記 `announced = true` 是安全的，避免重複報站；如果 snap 發生在站點後方很遠，`s_cm > stop.progress_cm + 4000` 就會直接設爲 `Departed`，這也沒問題。

---

## 4. 最終檢查清單（對應審查建議）

- ✅ `snapped` 信號使 snap 可被觀察  
- ✅ 前向 `find_forward_closest_stop_index` 避免選錯邊  
- ✅ FSM 重置使用幾何判斷（`AtStop`/`Departed`/`Approaching`）  
- ✅ 清除 `freeze_ctx`、重置 `prev_s_cm`  
- ✅ 2 秒 debounce（`just_snapped_ticks`）防止下一 tick 的 recovery 誤觸發  
- ✅ 同一 tick 內所有 recovery 路徑被 `!snapped` 與冷卻期阻擋  
- ✅ 意圖清晰：`snapped` 的語義明確，後續接手者不會混淆  

這個方案的結果是：**空間 snap 成爲唯一的狀態設置者，站點級 recovery 在 snap 後徹底沉默**。整個系統從「雙控制系統競爭」轉變爲明確的主從關係，具備生產環境的穩健性。

如果需要，我可以進一步提供完整的 diff 或修改後的完整檔案內容。