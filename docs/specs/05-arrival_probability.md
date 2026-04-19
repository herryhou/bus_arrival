# Arrival Probability Model Specification

## Overview
4-feature Bayesian model for arrival probability. Uses LUT-based Gaussian and logistic functions for no_std compatibility.

## Features

| Feature | Signal | Sigma | LUT Type |
|---------|--------|-------|----------|
| F1: Distance | z_gps_cm (raw GPS) | 2750 cm | Gaussian |
| F2: Speed | v_cms | - | Logistic |
| F3: Progress | s_cm (Kalman) | 2000 cm | Gaussian |
| F4: Dwell time | dwell_time_s | 10 s ref | Linear |

## Invariants (MUST)

- [ ] F1 uses RAW GPS (z_gps_cm), not Kalman (s_cm)
- [ ] F3 uses KALMAN (s_cm), not raw GPS
- [ ] Arrival threshold: THETA_ARRIVAL = 191 (75%)
- [ ] Standard weights: (13, 6, 10, 3) — sums to 32 for fast division
- [ ] Adaptive weights (close stop < 120 m): (14, 7, 11, 0) — remove p4
- [ ] Gaussian LUT: 256 entries, index = (x/sigma) × 64
- [ ] Logistic LUT: 128 entries, index = v / 10

## Formulas

**Standard weights:**
```rust
p = (13*p1 + 6*p2 + 10*p3 + 3*p4) / 32
```

**Adaptive weights (next_stop < 120 m):**
```rust
p = (14*p1 + 7*p2 + 11*p3 + 0*p4) / 32
```

**Gaussian LUT:**
```rust
fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    for i in 0..256 {
        let x = (i as f64) / 64.0;  // 0 to 4.0
        let g = (-0.5 * x * x).exp();
        lut[i] = (g * 255.0).min(255.0).round() as u8;
    }
    lut
}
```

## Version Notes

- v8.6: Adaptive weights for close stops (remove dwell time penalty)
- v8.4: PositionSignals separation (F1 vs F3)

## Related Files

- `crates/pipeline/detection/src/probability.rs` — Implementation
- `crates/shared/src/probability_constants.rs` — Constants
- `crates/shared/src/lib.rs` — PositionSignals definition
