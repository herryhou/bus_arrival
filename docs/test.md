根據文檔的架構，測試分四個層次：

**1. 單元測試（無需硬體，在開發機直接跑）**

```bash
# 在 workspace 根目錄
cargo test -p firmware
cargo test -p preprocessor
cargo test -p shared
```

firmware 是 no_std crate，但 `#[cfg(test)]` 區塊會自動切換到 host 環境編譯，不需要 Pico。

**2. 離線預處理驗證**

```bash
# 先跑 preprocessor，產出 route_data.bin
cargo run -p preprocessor -- input.geojson output/route_data.bin

# 確認產出符合預期
# 節點數 ~640、Flash ~34 KB、CRC 通過
```

**3. GPS Log 回放（準確率測試）**

這是最重要的整合測試。你需要先有真實路線的 NMEA log：

```bash
# 回放並計算 TP / FP / FN
node tools/replay.js test.nmea --ground-truth ground_truth.json
```

`ground_truth.json` 是人工標注的真實到站時間點，格式如：

```json
[
  { "stop_idx": 3, "timestamp": 1712345678 },
  { "stop_idx": 4, "timestamp": 1712345820 }
]
```

**4. 硬體上燒錄測試**

```bash
# 需安裝 probe-rs
cargo install probe-rs --features cli

# 編譯 + 燒錄
cargo build --release -p firmware
probe-rs run --chip RP2350 target/thumbv8m.main-none-eabihf/release/firmware

# 即時看 defmt 日誌
```

---

**建議開發順序：**

先把 `replay.py` 跑起來，在 host 上把準確率推到 ≥ 97% 再燒硬體，這樣 debug 速度快很多。

目前手上沒有 NMEA log 資料可以用來測試。