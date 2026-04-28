# 🧪 QA Review Report

## 🔴 Critical Design Flaws (C)

### C1. Hidden coupling between modules (state leakage across pipeline)

* **Issue:** Map matching → Kalman → detection → recovery are not cleanly decoupled.
* Evidence:

  * `last_known_stop_index` passed into map matching
  * recovery depends on Kalman `freeze_ctx`
* **Risk:** Emergent bugs (non-local reasoning), hard to validate correctness
* **Fix:** Define strict contracts:

  * Map matching = pure function of GPS + route
  * Recovery = separate reconciliation layer

---

### C2. Ambiguous source of truth for position (`s_cm` vs `z_gps_cm`)

* **Issue:** Multiple “truths”:

  * `z_gps_cm` (projection)
  * `s_cm` (Kalman output)
  * frozen `s_cm`
* Detection logic mixes them dynamically
* **Risk:** Non-deterministic behavior under edge cases
* **Fix:**

  * Define **single authoritative state per mode**:

    * Normal → Kalman (`s_cm`)
    * Divergence → fallback strategy (explicit mode switch)

---

### C3. Recovery logic overlaps with normal tracking

* Recovery triggered inside main loop based on heuristics
* Also triggered after off-route re-acquisition
* **Problem:** Competing mechanisms
* **Risk:** Oscillation / double-correction / index jumps
* **Fix:**

  * Introduce **explicit recovery state machine**
  * Ensure mutual exclusion:

    ```
    NORMAL ↔ OFF_ROUTE ↔ RECOVERING
    ```

---

### C4. Probabilistic model is not actually probabilistic

* Weighted sum:

  ```
  (13p1 + 6p2 + 10p3 + 3p4) / 32
  ```
* Called “Bayesian fusion” but:

  * No normalization
  * No independence assumption validation
* **Risk:** Misleading interpretation, hard tuning
* **Fix:**

  * Either:

    * Rename → “heuristic scoring model”
  * Or:

    * Convert to log-probability model

---

### C5. Lack of formal guarantees for monotonic progress

* You rely on:

  * speed constraint
  * Kalman smoothing
* But no **hard invariant**:

  ```
  s(t+1) >= s(t)
  ```
* **Risk:** backward jumps → wrong stop transitions
* **Fix:**

  * Enforce monotonic constraint at system boundary

---

## 🟠 Major Design Risks (M)

### M1. Over-optimization for no-FPU early

* Integer-only design everywhere
* Adds:

  * complexity
  * LUT maintenance
* **Risk:** premature optimization reduces clarity
* Suggest:

  * isolate math backend (int vs float interchangeable)

---

### M2. Grid + window + dual tracker complexity

* Map matching logic:

  * window search
  * grid fallback
  * eligible + any trackers
* **Risk:** hard to reason about correctness coverage
* Missing:

  * formal guarantee of candidate completeness

---

### M3. Corridor-based detection assumes perfect linearization

* Corridor uses:

  ```
  s_cm within [start, end]
  ```
* **Risk:**

  * geometry mismatch in sharp turns / parallel roads
* Should incorporate:

  * lateral distance constraint

---

### M4. Warmup logic intertwined with detection logic

* Warmup:

  * affects heading filter
  * blocks detection
* **Risk:** temporal coupling → hard debugging
* Suggest:

  * separate “estimation readiness” from “detection gating”

---

## 🟡 Implementation Issues (I)

### I1. Unsafe usage in persistence

```rust
core::ptr::read_unaligned(...)
```

* No versioning / schema evolution
* **Risk:** UB on struct change
* **Fix:** explicit serialization format

---

### I2. Potential overflow in weighted probability

```rust
(w1 * p1 + w2 * p2 + ...) / 32
```

* `u32` safe now, but no guard if weights change
* **Fix:** assert bounds or use checked math

---

### I3. Magic numbers scattered

Examples:

* `20000` (200m)
* `12000` (120m)
* `SIGMA_D_CM`, etc.
* **Risk:** tuning inconsistency
* **Fix:** central config module

---

### I4. Linear scan over stops every tick

```rust
for i in 0..route_data.stop_count
```

* O(N) each second
* Acceptable now, but not scalable
* **Fix:** index by progress (binary search / window)

---

### I5. Recovery trigger too naive

```rust
jump > 200m
```

* Ignores:

  * velocity
  * heading consistency
* **Risk:** false recovery under urban canyon
* **Fix:** multi-feature trigger

---

### I6. Heapless Vec silent truncation

```rust
if active.push(i).is_err() { break; }
```

* Drops data silently
* **Risk:** missing stops in dense routes
* **Fix:** explicit overflow handling

---

### I7. Tight coupling of firmware loop

* GPS read + processing + UART + persistence in one loop
* **Risk:** latency spikes, poor testability
* **Fix:** pipeline separation (tasks or stages)

---

### I8. State explosion risk

State struct includes:

* warmup
* recovery
* persistence
* snapping
* outage
* cooldown
* **Risk:** combinatorial bugs
* **Fix:** explicit state machine modeling

---

## 🟢 Strengths (S)

### S1. Excellent system decomposition

* Clear pipeline (offline / runtime / detection)

### S2. Strong embedded awareness

* Memory, CPU, LUT tradeoffs well handled

### S3. Deterministic design philosophy

* Very suitable for safety-critical embedded

### S4. Good handling of real-world GPS issues

* drift / jump / outage all addressed

---

# 🧭 Final Verdict

### Overall Quality: **High (but complex and fragile)**

* Architecture is **well thought-out**
* But system suffers from:

  * **implicit coupling**
  * **state complexity**
  * **heuristic accumulation**

---

# 🎯 Top 3 Fix Priorities

1. **C1 + C3 → Separate concerns**

   * decouple recovery / matching / detection

2. **C2 → Define single source of truth**

   * eliminate mixed signal usage

3. **C4 → Fix probability model semantics**

   * either formalize or simplify

---

If you want, I can:

* Refactor this into a **clean layered architecture (DDD style)**
* Or redesign the **state machine to remove hidden coupling**
* Or deep-dive one module (e.g., map matching correctness proof)
