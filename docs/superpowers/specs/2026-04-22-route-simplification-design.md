# 📘 Route Simplification Design Guide

**Name:** Constraint-Aware Error-Bounded Simplification (CAEBS v2)
**Purpose:** Preprocess route polylines for downstream systems (M2–M5) with strict guarantees

---

## 1. 🎯 Problem Definition

Given a route:

```text
P = [P₀, P₁, ..., Pₙ]
```

Produce a simplified route:

```text
S = [S₀, S₁, ..., Sₘ],  m << n
```

---

### 1.1 Objectives

* Reduce point count
* Preserve geometric fidelity
* Preserve critical features (stops, turns)
* Satisfy downstream constraints (segment length, monotonic distance)

---

### 1.2 Hard Constraints

For every segment `(Sᵢ, Sᵢ₊₁)`:

#### (A) Segment length

```text
General zone: ≤ 100m
Critical zones: ≤ 30m
```

#### (B) Geometric error

```text
General: ≤ 7m
Sharp turns: ≤ 2.5m
```

#### (C) Stop preservation

```text
All points within ±30m of stops MUST be preserved
```

#### (D) Monotonicity

```text
cum_dist[i+1] > cum_dist[i]
(no zero-length segments)
```

---

## 2. 🧠 Core Design

---

### 2.1 Zones (critical abstraction)

Each segment belongs to:

| Zone      | Definition           |
| --------- | -------------------- |
| General   | default              |
| Stop zone | within ±100m of stop |
| Turn zone | contains sharp turn  |

---

### 2.2 Zone parameters

| Zone    | ε    | L_max |
| ------- | ---- | ----- |
| General | 7m   | 100m  |
| Stop    | 7m   | 30m   |
| Turn    | 2.5m | 30m   |

---

### 2.3 Anchors

Anchors are always preserved:

```text
- start/end
- stop indices
- all points within ±30m of stops
- sharp turn points
```

---

## 3. 🔁 Algorithm

---

### 3.1 Preprocessing

1. Compute cumulative distance
2. Detect sharp turns
3. Expand stop zones (±30m and ±100m)
4. Build anchors

---

### 3.2 Partition

Split route by anchors:

```text
[A₀ → A₁], [A₁ → A₂], ...
```

---

### 3.3 Constrained DP

For segment `(i, j)`:

```text
d_max = max perpendicular distance
L = cumulative_distance[j] - cumulative_distance[i]

zone = classify(i, j)

if (d_max ≤ ε(zone)) AND (L ≤ L_max(zone)):
    keep endpoints
else:
    if d_max > ε:
        split at argmax error
    else:
        split at midpoint   ← deterministic fix
```

---

### 3.4 Post-processing

* Remove duplicate points
* Ensure monotonic cumulative distance

---

## 4. 🦀 Rust Implementation

---

### 4.1 types.rs

```rust
##[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub type Route = Vec<Point>;

##[derive(Debug, Clone, Copy)]
pub enum Zone {
    General,
    Stop,
    Turn,
}

##[derive(Debug, Clone)]
pub struct SimplifyConfig {
    pub epsilon_general: f64,   // 7.0
    pub epsilon_turn: f64,      // 2.5

    pub max_len_general: f64,   // 100.0
    pub max_len_critical: f64,  // 30.0

    pub stop_indices: Vec<usize>,
    pub turn_threshold_deg: f64,
}
```

---

### 4.2 distance.rs

```rust
pub fn dist(a: Point, b: Point) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

pub fn perpendicular_distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;

    if dx == 0.0 && dy == 0.0 {
        return dist(p, a);
    }

    let mut t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy);

    // FIX: segment projection
    t = t.clamp(0.0, 1.0);

    let proj = Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    };

    dist(p, proj)
}

pub fn cumulative_distances(points: &[Point]) -> Vec<f64> {
    let mut d = vec![0.0; points.len()];
    for i in 1..points.len() {
        d[i] = d[i - 1] + dist(points[i - 1], points[i]);
    }
    d
}
```

---

### 4.3 anchors.rs

```rust
pub fn detect_sharp_turns(points: &[Point], threshold_deg: f64) -> Vec<usize> {
    let mut out = Vec::new();

    for i in 1..points.len() - 1 {
        let a = points[i - 1];
        let b = points[i];
        let c = points[i + 1];

        let v1 = (b.x - a.x, b.y - a.y);
        let v2 = (c.x - b.x, c.y - b.y);

        let dot = v1.0 * v2.0 + v1.1 * v2.1;
        let mag1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt();
        let mag2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt();

        if mag1 == 0.0 || mag2 == 0.0 {
            continue;
        }

        let cos = (dot / (mag1 * mag2)).clamp(-1.0, 1.0);
        let theta = cos.acos().to_degrees();

        if theta > threshold_deg {
            out.push(i);
        }
    }

    out
}
```

---

### 4.4 zones.rs

```rust
pub fn expand_stop_zone(cum: &[f64], stops: &[usize], radius: f64) -> Vec<bool> {
    let mut mask = vec![false; cum.len()];

    for &s in stops {
        if s >= cum.len() { continue; }
        let d0 = cum[s];

        for i in 0..cum.len() {
            if (cum[i] - d0).abs() <= radius {
                mask[i] = true;
            }
        }
    }

    mask
}

pub fn classify_zone(
    i: usize,
    j: usize,
    stop_mask: &[bool],
    turn_mask: &[bool],
) -> Zone {
    if (i..=j).any(|k| stop_mask[k]) {
        Zone::Stop
    } else if (i..=j).any(|k| turn_mask[k]) {
        Zone::Turn
    } else {
        Zone::General
    }
}
```

---

### 4.5 dp.rs

```rust
pub fn simplify_segment(
    pts: &[Point],
    cum: &[f64],
    i: usize,
    j: usize,
    cfg: &SimplifyConfig,
    stop_mask: &[bool],
    turn_mask: &[bool],
    keep: &mut Vec<bool>,
) {
    if j <= i + 1 {
        return;
    }

    let zone = classify_zone(i, j, stop_mask, turn_mask);

    let eps = match zone {
        Zone::Turn => cfg.epsilon_turn,
        _ => cfg.epsilon_general,
    };

    let max_len = match zone {
        Zone::General => cfg.max_len_general,
        _ => cfg.max_len_critical,
    };

    let mut max_dist = 0.0;
    let mut idx = i;

    for k in (i + 1)..j {
        let d = perpendicular_distance(pts[k], pts[i], pts[j]);
        if d > max_dist {
            max_dist = d;
            idx = k;
        }
    }

    let seg_len = cum[j] - cum[i];

    let (split_idx, split) = if max_dist > eps {
        (idx, true)
    } else if seg_len > max_len {
        ((i + j) / 2, true) // FIX
    } else {
        (0, false)
    };

    if !split {
        return;
    }

    keep[split_idx] = true;

    simplify_segment(pts, cum, i, split_idx, cfg, stop_mask, turn_mask, keep);
    simplify_segment(pts, cum, split_idx, j, cfg, stop_mask, turn_mask, keep);
}
```

---

### 4.6 pipeline.rs

```rust
pub fn simplify_route(route: &Route, cfg: &SimplifyConfig) -> Route {
    let n = route.len();
    if n <= 2 {
        return route.clone();
    }

    let cum = cumulative_distances(route);
    let turns = detect_sharp_turns(route, cfg.turn_threshold_deg);

    let stop_mask_30 = expand_stop_zone(&cum, &cfg.stop_indices, 30.0);
    let stop_mask_100 = expand_stop_zone(&cum, &cfg.stop_indices, 100.0);

    let mut turn_mask = vec![false; n];
    for &t in &turns {
        if t < n {
            turn_mask[t] = true;
        }
    }

    let mut keep = vec![false; n];

    // anchors
    keep[0] = true;
    keep[n - 1] = true;

    for &s in &cfg.stop_indices {
        if s < n {
            keep[s] = true;
        }
    }

    for i in 0..n {
        if stop_mask_30[i] {
            keep[i] = true;
        }
        if turn_mask[i] {
            keep[i] = true;
        }
    }

    simplify_segment(
        route,
        &cum,
        0,
        n - 1,
        cfg,
        &stop_mask_100,
        &turn_mask,
        &mut keep,
    );

    let mut out: Vec<Point> = route
        .iter()
        .enumerate()
        .filter_map(|(i, p)| if keep[i] { Some(*p) } else { None })
        .collect();

    // remove duplicates
    out.dedup();

    out
}
```

---

## 🧪 Test Strategy

---

### 1. Constraint: max segment length

```rust
##[test]
fn max_segment_length_respected() {
    let route = long_straight_line(300.0);

    let result = simplify(route);

    for w in result.windows(2) {
        assert!(dist(w[0], w[1]) <= 100.0 + 1e-6);
    }
}
```

---

### 2. Sharp turn preservation

```rust
##[test]
fn preserves_sharp_turn() {
    let route = sharp_turn_example();

    let result = simplify(route);

    assert!(contains_turn(&result));
}
```

---

### 3. Stop zone preservation

```rust
##[test]
fn preserves_stop_zone_points() {
    let route = route_with_dense_stop();

    let result = simplify(route);

    assert!(points_within_30m_preserved(&route, &result));
}
```

---

### 4. Monotonicity

```rust
##[test]
fn cumulative_distance_monotonic() {
    let result = simplify(random_route());

    let cum = cumulative_distances(&result);

    for i in 1..cum.len() {
        assert!(cum[i] > cum[i - 1]);
    }
}
```

---

### 5. Property-based

```rust
proptest! {
    #[test]
    fn error_bounded(route in random_routes()) {
        let simplified = simplify(route.clone());

        assert!(max_error(&route, &simplified) <= EPSILON + 1e-6);
    }
}
```

---

## 🧠 Final Takeaway

This system is now:

* **Constraint-first (not heuristic)**
* **Zone-aware (core concept)**
* **Deterministic**
* **Spec-compliant**
* **Safe for downstream (M2–M5)**
