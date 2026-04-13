
## E1. The Heading Penalty Has No Tunable λ — It Silently Overrides Distance

The spec defines:

$$\text{score}(i) = d^2 + \lambda \cdot \text{diff}^2 \cdot w_h$$

and recommends λ = 1 cm²/cdeg² as a starting value. In `segment_score`, the actual code computes:

```rust
((heading_diff as i64).pow(2) * w as i64) >> 8
```

λ is absent (effectively λ = 1, unitless). The problem is a unit mismatch: `heading_diff` is in centidegrees (0.01°), so a 90° difference = 9,000 cdeg, giving a heading term of 9000² × 256 >> 8 = **324,000,000**. A typical on-route `d²` for a bus 10–50 cm off the line is **100–25,000**. The heading term is 10,000× larger than the distance term, turning a soft penalty into a de facto hard exclusion. Near stops where GPS heading is unreliable (bus nearly stationary, heading jitter), this will frequently cause wrong segment selection because the low-speed weight ramp never fully zeroes the term — it approaches zero but the quadratic heading penalty is so large that even a 10% residual dominates.

The λ parameter needs to have units of cm²/cdeg² with a value of ~0.000003 to produce a numerically balanced score, or the entire heading term needs to be rethought with proper unit reconciliation.

---

## E2. The Probability Model Is a Linear Discriminant, Not Bayesian Fusion

The spec describes the probability model as "Bayesian fusion" but the formula is:

$$P = \frac{13p_1 + 6p_2 + 10p_3 + 3p_4}{32}$$

This is a weighted linear sum — a linear discriminant. Bayesian fusion would multiply the likelihoods: $P \propto p_1 \cdot p_2 \cdot p_3 \cdot p_4$. The choice of a linear sum vs multiplicative fusion has a concrete operational difference: in a linear sum, a single very high feature (e.g. p3 = 255 because Kalman says exactly at the stop) can push P above threshold even if p2 = 0 (bus moving fast) and p4 = 0 (just entered corridor). Multiplicative fusion would block this because any near-zero term collapses the product. The current design can be fooled by a single strong feature while others contradict arrival. Whether this is desirable is a design choice, but calling it "Bayesian" obscures the actual logic and would mislead developers trying to calibrate it.

---

## E3. F1 and F3 Are Structurally Correlated — Two Features for the Price of None

The spec's rationale: F1 = raw GPS projection (noisy, independent), F3 = Kalman-smoothed position (stable, correlated across time). The intent is two independent views of the same physical fact.

The flaw: both F1 and F3 are computed from `|s_cm - stop.progress_cm|` using the **same** `s_cm`. Beyond the implementation bug already noted in the code review, there is a deeper design problem: even with a correctly-passed raw `z_gps`, F1 and F3 will be highly correlated — F3 is a smoothed version of F1. The Kalman filter's job is to reduce noise in F1. Adding F3 as a separate feature doesn't contribute independent information; it contributes a smoother repeat of F1. The linear discriminant's effective rank is therefore lower than 4, and you're wasting weight (10 out of 32) on a near-redundant feature. A better design would use F3 as a **replacement** for F1 when GPS quality is poor (HDOP-switched), not as an additive term.

---

## E4. HDOP Is Always One Message Cycle Stale

The NMEA parsing sequence in practice: RMC → GGA → GSA (or RMC → GSA, or GGA alone).

`parse_gga` calls `core::mem::replace`, which immediately returns the completed point and resets internal state. This happens **before** GSA is parsed. The HDOP from GSA ends up in the *next cycle's* point. The HDOP-adaptive Kalman gain (Section 10.4.1) is therefore always using last cycle's quality metric. In a city-canyon where quality degrades suddenly (the bus enters a building shadow), the Kalman gain doesn't respond until the following second — the exact tick where reduced trust matters most gets the stale high-trust gain.

The fix is to hold the point in a staging buffer until both a position source and a HDOP source have been received in the same burst, with a fallback if only GGA is available.

---

## E5. Midnight Rollover Causes DR to Stall

GPS timestamps are derived from HHMMSS as `hh * 3600 + mm * 60 + ss` — a value in `[0, 86399]`. Dead-reckoning computes:

```rust
let dt = timestamp.saturating_sub(last_gps_time);
```

At midnight, `last_gps_time = 86399` and `timestamp = 0`. `(0u64).saturating_sub(86399)` = 0. `dt = 0`, so DR advances position by zero, and the decay factor `DR_DECAY_NUMERATOR[0] = 10000` leaves speed unchanged. The bus effectively freezes in the DR model for one tick. On a 1 Hz system this is minor, but a service running the overnight shift will silently stall position for one second at exactly 00:00:00.

---

## E6. Only One Event Returns Per Tick — Silent Drop When Corridors Overlap

`process_gps` in `state.rs` iterates over active stops and returns `Some(...)` immediately on the first `StopEvent::Arrived` or announcement. If two corridors overlap (which the close-stop logic explicitly creates), and both trigger an event in the same tick, only the lower-indexed stop's event is returned. The higher-indexed stop's FSM is updated (state transitions correctly), but its arrival event is dropped. The caller never knows it happened. This is a structural limitation of returning `Option<ArrivalEvent>` rather than a `Vec<ArrivalEvent>` or a callback.

---

## E7. Recovery Triggers Cannot Fire Even If Wired Up

The spec lists three Recovery triggers (Section 15.1): GPS jump > 200 m, restart mismatch > 500 m, and sustained position/stop divergence > 5 s. None of the state needed to detect these is tracked in `State`:

- No previous `s_cm` snapshot to detect a 200 m jump (the Kalman filter absorbs or rejects the jump without recording a "before" value)
- No divergence timer counting consecutive ticks where `|ŝ - s_i| > 200 m`
- `PersistedState` doesn't exist for restart comparison

Before Module ⑫ can be wired up, three new fields must be added to `State` and their update logic written. Recovery is not just "missing wiring" — the architectural prerequisites for triggering it are absent.

---

## E8. The One-Time Announcement Rule Breaks Circular Routes

Section 14.4 makes `announced = true` permanent for the lifetime of a `StopState`. For a circular route (bus loops back past stop #5 twice per trip), the second pass is silent. The spec acknowledges this as "expected behavior." But many real transit routes are circular, and this architectural decision makes the system unsuitable for them without a trip-reset mechanism. The spec doesn't define what constitutes a "trip boundary" or how to detect one, leaving this as an unresolved operational gap.

---

## E9. The Close-Stop Corridor Adjustment and Adaptive Weights Aren't Jointly Calibrated

The close-stop fix (v8.6) applies two independent interventions simultaneously: corridor resizing (Section 12.5) and weight redistribution from (13,6,10,3) to (14,7,11,0). These two changes interact: the resized corridor means dwell time accumulates differently, but the adaptive weights remove dwell time entirely for those stops. The system was validated on a single real-world case (tpF805 Stop #2→#3 at 79 m), and probability went from 185 → 222. However, the threshold θ=191 was set under the standard weights. With adaptive weights that remove p4 entirely, the score distribution shifts. Whether θ=191 remains the right threshold for the (14,7,11,0) regime was never independently evaluated — the validation just confirms "> 191," not that the threshold is still optimal.

---

## E10. The 80 m Pre-Corridor Is Both a Feature and a Constraint — and Creates an Edge Case at Route Start

The corridor's pre-corridor width (80 m) serves two roles: it's the announcement trigger, and it's the window for dwell-time accumulation. These two purposes impose conflicting constraints. For announcement you want the corridor to start early (before the bus decelerates). For dwell-time you want it to start late (to avoid counting slow-approach time as dwell time). The system resolves this by making the corridor serve both, but the result is that `τ_dwell` and `T_ref = 10 s` must be tuned relative to corridor width and bus approach speed simultaneously — they aren't independent parameters.

Additionally, the first stop on the route has `corridor_start = stop_progress - 8000 cm`. If the route starts near the depot and the first stop is less than 80 m from the route origin, `corridor_start_cm` would be negative. The `find_active_stops` check `s_cm >= stop.corridor_start_cm` would then fire the moment the system starts (s_cm ≈ 0), triggering an announcement before the bus has moved. The preprocessor's corridor overlap logic doesn't check for negative `corridor_start_cm`.

---

## Summary

| Issue | Impact |
|-------|--------|
| Heading penalty lacks λ — dominates distance at 10,000× | Wrong segment selected near stops, especially during approach/low-speed |
| Linear sum mislabeled as Bayesian — single feature can override others | False positives when one feature fires strongly and others contradict |
| F1 and F3 are structurally correlated | Wastes 10/32 weight on near-redundant signal |
| HDOP one cycle stale | Adaptive Kalman responds too late on rapid quality degradation |
| Midnight rollover — DR freezes for one tick | Silent position stall at 00:00:00 |
| Only one event per tick — second corridor event silently dropped | Missed arrival reports on close stops |
| Recovery trigger state missing from `State` struct | Module ⑫ is unimplementable without architectural additions |
| One-time rule breaks circular routes | System is unsuitable for loop routes without trip-reset design |
| Close-stop threshold θ=191 not re-validated under adaptive weights | Threshold may be wrong for (14,7,11,0) regime |
| First stop may have negative corridor start | Spurious announcement at route initialization |