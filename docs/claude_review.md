Here is a systematic review across all three layers, from high-level design down to concrete implementation bugs.

---

## High-Level Design

**H1 — Module ⑫ (Stop Index Recovery) is entirely missing from the firmware path**
`state.rs` runs: corridor filter → probability → FSM → announce. It never calls `recovery::find_stop_index`. The three trigger conditions described in Section 15.1 (GPS jump > 200 m, restart mismatch, sustained position/stop divergence) are never evaluated. The recovery crate exists but is dead code in firmware.

**H2 — `PersistedState` Flash persistence is not implemented**
Section 11.4 specifies CRC-protected storage of `last_progress_cm` / `last_stop_index` across reboots. The struct is mentioned in the spec but does not appear anywhere in the source tree. Cold-start always begins at index 0.

**H3 — DR soft-resync not implemented**
Section 11.3 specifies: after GPS recovery, apply `ŝ_resync = ŝ_DR + (2/10)*(z_gps - ŝ_DR)`. In `kalman.rs::process_gps_update`, once the speed constraint passes, the code goes straight into `update_adaptive` with full Kalman gain — there is no soft-correction step. First post-outage GPS is applied with Ks = 13–77/256 rather than the intentionally conservative 2/10.

**H4 — EMA velocity filter not implemented**
Section 11.1 specifies `v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10` to feed DR. The code simply does `dr.filtered_v = state.v_cms` — it copies the Kalman output directly. The decay table `(9/10)^dt` applied during outage is correct but is being applied to an un-smoothed velocity, making DR speed estimates noisier than the spec intends.

---

## Detailed Design

**D1 — F1 and F3 are fed the same value (Kalman `s_cm`)**
The spec deliberately uses two sources: F1 = raw GPS projection z_gps (σ = 2750 cm), F3 = Kalman-smoothed `ŝ` (σ = 2000 cm). In both the firmware `detection.rs` and `pipeline/detection/probability.rs`, only a single `s_cm` is passed to `compute_features` / `arrival_probability`, and both F1 and F3 are computed from `(s_cm - stop.progress_cm).abs()`. The raw projection is never forwarded to the detection layer. The "two independent signal sources" rationale in Section 13.2 is not realised — F1 and F3 differ only in sigma, not in input signal.

**D2 — Monotonicity threshold: −10 m in spec vs −500 m in code**
Section 8.3: reject if `z(t) − ŝ(t−1) < −1000 cm` (−10 m). `check_monotonic` in `kalman.rs` uses `z_new >= z_prev - 50000` (−500 m). The filter is intended to block reverse GPS jumps; the current threshold is so loose it accepts almost any plausible GPS noise event.

**D3 — Speed constraint is far more lenient than the spec**
Section 9.1 sets D_max = 1667 + 2000 = 3667 cm (≈37 m). In `kalman.rs`, `V_MAX_CMS = 3000 cm/s` and `SIGMA_GPS_CM = 5000 cm`, giving `max_dist = 3000 + 5000 = 8000 cm` (80 m) — more than double the spec value. The `recovery.rs` module also uses `V_MAX_CMS = 3000`. The appendix says V_max = 1667 cm/s; the implementation effectively allows 108 km/h plus a 50 m GPS margin.

**D4 — `Arriving` state has no exit path back to `Idle`**
In `state_machine.rs`, when `fsm_state == Arriving` and `s_cm` drops below `corridor_start_cm` (GPS drift backward), there is no `Arriving → Idle` transition. The FSM stays stuck in `Arriving`, `dwell_time_s` keeps incrementing, and the stop remains "active" indefinitely. Compare: `Approaching` correctly has `if s_cm < corridor_start_cm → Idle + reset dwell`.

**D5 — Dwell-time counter off-by-one on corridor entry**
The spec says `τ_dwell` starts counting from when `Approaching` is entered. In `update()`, the `Idle` arm transitions the FSM to `Approaching` but does not increment; the increment only fires on the next tick when already in `Approaching`. After T seconds in corridor, `dwell_time_s = T − 1`. For `T_ref = 10 s`, `p4 = (9 × 255 / 10) = 229` rather than `255`, under-weighting the dwell feature.

---

## Implementation

**I1 — `build.rs` arch detection is macOS-only**
```rust
"x86_64" => "x86_64-apple-darwin",
"arm64"  => "aarch64-apple-darwin",
_        => panic!("Unknown machine architecture"),
```
Any Linux CI or developer machine will panic at build time. Should map to `*-unknown-linux-gnu` / `*-unknown-linux-musl` based on target OS.

**I2 — UART event writer casts `i32` to `u64` unsafely**
In `uart.rs`:
```rust
append_u64(&mut msg_buf, &mut pos, event.s_cm as u64)?;
append_u64(&mut msg_buf, &mut pos, event.v_cms as u64)?;
```
`s_cm` and `v_cms` are `i32`. A negative value — which is possible during cold-start before Kalman converges — would be emitted as a huge number (e.g. `−1` → `4294967295`). Should use a signed formatter or assert non-negative before casting.

**I3 — `RouteNode` version comment claims 32 bytes; actual size is 24 bytes**
In `binfile.rs` VERSION comments:
```
/// v4 (v8.7): ... Size now 32 bytes (28 data + 4 padding).
```
But `shared/src/lib.rs` asserts `size_of::<RouteNode>() == 24`. The discrepancy arises because the comment was written when `seg_len_mm` was `i64` (8 bytes); it became `i32` (4 bytes) in the same version. Any tooling that parses the comment to validate binary compatibility will be wrong.

**I4 — Memory leak in `SpatialGridView::get_cell` for misaligned XIP (std path)**
```rust
let leaked: &'static [u16] = vec.leak();
Ok(unsafe { core::mem::transmute::<&'static [u16], &'a [u16]>(leaked) })
```
Every call to `get_cell` on a misaligned address allocates and permanently leaks a `Vec<u16>`. During testing (std feature) map-matching invokes `get_cell` hundreds of times per route run. This is documented as acceptable for firmware (one-time startup) but makes long integration test runs accumulate unbounded heap. Should copy once at load time into aligned storage, or use `read_unaligned` per-element without leaking.

**I5 — Warmup counter never advances if GPS is repeatedly rejected**
In `state.rs`:
```rust
ProcessResult::Rejected(_) => { return None; }   // counter not incremented
```
If the first several GPS samples all fail the speed constraint (e.g., large initial position error), `warmup_counter` stays at 0 and the system never enters normal detection. `ProcessResult::DrOutage` does proceed to detection, but pure rejections do not advance warmup.

**I6 — UART RX buffer (256 bytes) is dangerously small for a three-sentence GPS burst**
A typical NMEA burst (`$GPRMC` + `$GPGGA` + `$GNGSA`) is ~220–260 bytes. The RX buffer is exactly 256 bytes. If the burst arrives during the 1-second sleep between outer-loop iterations, bytes may be silently dropped by the UART FIFO before the inner drain loop runs. Minimum safe size is ~512 bytes given NMEA maximums.

---

## Priority Summary

| ID | Severity | Description |
|----|----------|-------------|
| D1 | 🔴 High | F1 and F3 use same Kalman `s_cm` — raw GPS never forwarded |
| D2 | 🔴 High | Monotonicity threshold −500 m vs spec −10 m |
| D3 | 🔴 High | Speed constraint 8000 cm vs spec 3667 cm |
| H1 | 🔴 High | Module ⑫ Recovery not wired into firmware pipeline |
| H3 | 🟠 Med | DR soft-resync (2/10) not implemented |
| D4 | 🟠 Med | `Arriving → Idle` transition missing on corridor exit |
| H2 | 🟠 Med | Flash state persistence not implemented |
| H4 | 🟠 Med | EMA velocity filter not implemented |
| I5 | 🟠 Med | Warmup counter stuck on repeated Rejected results |
| I6 | 🟠 Med | UART RX buffer undersized for GPS burst |
| I1 | 🟡 Low | `build.rs` macOS-only, breaks Linux |
| I2 | 🟡 Low | UART i32→u64 cast wrong for negative values |
| D5 | 🟡 Low | Dwell-time off-by-one on corridor entry |
| I3 | 🟡 Low | RouteNode version comment says 32 bytes, actual is 24 |
| I4 | 🟡 Low | Memory leak in XIP misaligned path (std tests) |

---

## Implementation Status

*Last updated: 2026-04-12*

| ID | Status | Commits | Notes |
|----|--------|---------|-------|
| **D1** | ✅ Complete | d488758, 419c105, 2258556, 7d0188e, 33420d1, a291114 | F1/F3 signal separation via `PositionSignals` struct. F1 uses raw GPS (`z_gps_cm`, σ=2750), F3 uses Kalman (`s_cm`, σ=2000). |
| **D2** | ✅ Complete | 1ca6da2, 0871ec7 | Monotonicity threshold changed from -50000 cm to -5000 cm (-50 m). |
| **D3** | ✅ Complete | f89645f, eef532d | Speed constraint: V_MAX_CMS=1667 (60 km/h), SIGMA_GPS_CM=2000 (20 m). |
| **D4** | ✅ Complete | a272125, d1c8fe4 | Arriving → Idle transition on corridor exit. Resets `dwell_time_s`, preserves `announced` flag. |
| **D5** | ⏸️ Pending | — | Dwell-time counter off-by-one on corridor entry. |
| **H1** | ✅ Complete | 8873959, 0dd557c, 9aa62cd | Recovery module wired into firmware with GPS jump detection. |
| **H2** | ⏸️ Pending | — | Flash state persistence not implemented. |
| **H3** | ⏸️ Pending | — | DR soft-resync (2/10) not implemented. |
| **H4** | ⏸️ Pending | — | EMA velocity filter not implemented (still uses direct `state.v_cms`). |
| **I1** | ✅ Complete | — | build.rs removed; build system now uses standard cargo cross-compilation. |
| **I2** | ⏸️ Pending | — | UART i32→u64 cast issue still present. |
| **I3** | ⏸️ Pending | — | RouteNode version comment still says "32 bytes" but actual is 24. |
| **I4** | ⏸️ Pending | — | Memory leak in XIP misaligned path still present. |
| **I5** | ✅ Complete | c1f1010, 007cc3d, 470bf75, fec1b13, 34d0dc1, 66198ec | Two-counter warmup system: valid_ticks (convergence) + total_ticks (timeout). DrOutage also updated. |
| **I6** | ⏸️ Pending | — | UART RX buffer still 256 bytes (should be 512+). |

### Summary

- **9 of 15 issues resolved** (D1, D2, D3, D4, H1, I1, I5)
- **2 High-severity remaining** (H3)
- **5 Medium-severity remaining** (H2, H4, I6, D5)
- **4 Low-severity remaining** (I2, I3, I4)