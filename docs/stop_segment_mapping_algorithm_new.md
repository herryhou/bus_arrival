## The Fundamental Flaw: Greedy is Wrong by Design

Before proposing a new algorithm, let me articulate **why the greedy approach is structurally broken**, not just buggy.

The greedy picks the closest valid segment *for stop j* without any knowledge of where stop j+1 is. A locally optimal mapping for stop j can make stop j+1's mapping catastrophically worse — and no amount of edge-case patching fixes this.

```
Stop 1: Two candidates
  A → seg=5, dist=8m   ← greedy picks this
  B → seg=3, dist=12m

Stop 2: Two candidates  
  C → seg=4, dist=5m   ← now blocked by A (seg 4 < seg 5)
  D → seg=6, dist=80m  ← only valid option after greedy picked A

Greedy total cost:   8 + 80 = 88m
Optimal (pick B→C):  12 + 5 = 17m
```

The greedy is 5× worse here. This is not an edge case — it happens wherever a route doubles back or stops are dense relative to segment length.

---

## New Approach: Reduce to a DAG Shortest Path (Viterbi)

You've already built a Viterbi decoder for the bus arrival HMM. This problem has **exactly the same structure**.

### Conceptual Model

- Each stop `j` has `K` **candidate projections** (top-K closest segments from grid search)
- Candidates form layers in a DAG
- An edge from candidate `(j-1, a)` to `(j, b)` exists iff `progress[b] >= progress[a]`
- Edge weight = `dist(stop[j], projection[b])`
- **Find the minimum-cost path through the DAG**

```
Layer 0        Layer 1        Layer 2
(stop 0)       (stop 1)       (stop 2)

 [cand A] ───▶ [cand C] ───▶ [cand F]
    │       ╲      │       ╲     │
    │        ╲     │        ╲    │
 [cand B] ───▶ [cand D] ───▶ [cand G]
                   │
                [cand E] ──▶ (no valid successor → skip)
```

The path cost is the **sum of distances** from each stop to its chosen candidate. This is globally optimal.

---

## Algorithm

### Phase 1: Candidate Generation

```
FUNCTION GENERATE_CANDIDATES(stop, route, grid, K):
    candidates = []

    // Grid query — all 3 radii, no early exit
    seen = {}
    FOR radius = 1 TO 3:
        FOR seg_idx IN QUERY_GRID(grid, stop, radius):
            IF seg_idx IN seen: CONTINUE
            seen.add(seg_idx)
            t_raw = PROJECT_UNCLAMPED(stop, route[seg_idx])
            t_val = CLAMP(t_raw, 0.0, 1.0)
            P     = route[seg_idx].A + t_val * route[seg_idx].AB
            dist  = |stop - P|
            prog  = route[seg_idx].cum_dist + t_val * route[seg_idx].seg_len
            candidates.push({seg: seg_idx, t: t_val, dist: dist, progress: prog})

    // Deduplicate and keep top-K by distance
    SORT candidates BY dist ASC
    RETURN candidates[0 : K]
```

Key: we keep **K candidates per stop**, not just 1. K=10–15 is sufficient in practice.

### Phase 2: DP Over the DAG

The transition constraint is just `progress[b] >= progress[a]` — a single scalar comparison. No two-dimensional coupling, no `(seg, t)` state, no edge cases.

```
FUNCTION MAP_STOPS_DP(route, stops, grid, K=15):
    M = length(stops)

    // Generate candidates for all stops
    cands = [GENERATE_CANDIDATES(stops[j], route, grid, K) for j in 0..M-1]

    // dp[j][k] = minimum total distance to assign stops 0..j
    //            with stop j mapped to candidate k
    // prev[j][k] = index of best predecessor candidate in layer j-1

    dp   = M × K matrix, initialized to +∞
    prev = M × K matrix, initialized to -1

    // Base case: first stop, no constraint
    FOR k = 0 TO len(cands[0])-1:
        dp[0][k] = cands[0][k].dist

    // Forward pass
    FOR j = 1 TO M-1:
        // Sort current layer by progress to enable O(K) sweep
        // (candidates already sorted by dist; sort a copy by progress)
        curr = SORT_BY_PROGRESS(cands[j])

        // Running minimum over previous layer, sorted by progress
        prev_sorted = SORT_BY_PROGRESS(cands[j-1])  // with original indices
        ptr = 0
        best_prev_cost = +∞
        best_prev_k    = -1

        FOR each candidate c IN curr (increasing progress order):
            // Advance pointer: include all prev candidates with progress <= c.progress
            WHILE ptr < len(prev_sorted) AND prev_sorted[ptr].progress <= c.progress:
                p = prev_sorted[ptr]
                IF dp[j-1][p.orig_idx] < best_prev_cost:
                    best_prev_cost = dp[j-1][p.orig_idx]
                    best_prev_k    = p.orig_idx
                ptr++

            IF best_prev_cost < +∞:
                cost = best_prev_cost + c.dist
                IF cost < dp[j][c.orig_idx]:
                    dp[j][c.orig_idx] = cost
                    prev[j][c.orig_idx] = best_prev_k

    // Find best final candidate
    best_k = argmin over k of dp[M-1][k]

    // Backtrack
    results = array of size M
    k = best_k
    FOR j = M-1 DOWNTO 0:
        results[j] = cands[j][k]
        k = prev[j][k]

    RETURN results
```

### Phase 3: Fallback for Disconnected Layers

If no transition exists into a layer (all candidates for stop `j` have progress < best progress after stop `j-1`), we need a snap forward. Rather than patching the DP, add **one extra synthetic candidate** per stop:

```
// At end of GENERATE_CANDIDATES, always append:
snap_seg = first segment where cum_dist + seg_len >= min_known_prog
snap_cand = {seg: snap_seg, t: 0.0, dist: LARGE_PENALTY, progress: ...}
candidates.push(snap_cand)
```

The large penalty ensures the DP only uses it when no natural candidate is valid. This is the only place a snap appears — and it's isolated, not woven through the core logic.

---

## How Every Previous Edge Case Dissolves

| Edge Case | Greedy Handling | DP Handling |
|---|---|---|
| Stop projects behind `min_prog` on current segment | Snap computation with epsilon guard | DP simply doesn't use that candidate; snap is a single fallback candidate |
| Greedy locally optimal but globally suboptimal | **Structurally wrong** | DP is globally optimal by construction |
| Route loop (same location visited twice) | Grid returns both segments; t-constraint tries to pick later one | Both segments appear as candidates in the layer; DP picks the globally best sequence |
| Identical adjacent stops | Strict `>` forces advancement | Equal progress is a valid DP transition `>=` |
| Long segment not in grid cells | BBox expansion at index build time | Still needed — candidate generation quality matters, but DP correctness doesn't depend on it |
| FP boundary precision | Epsilon guard in `t_lo` | Constraint is `progress_a <= progress_b` — no division, no boundary snap in hot path |

---

## Complexity

| Phase | Cost |
|---|---|
| Candidate generation | `O(M × K)` |
| Per-layer sort | `O(M × K log K)` |
| DP sweep | `O(M × K)` |
| Backtrack | `O(M)` |
| **Total** | **`O(M × K log K)`** |

With `M=35` stops and `K=15` candidates: ~525 projection evaluations + trivial sort. This is faster than the linear fallback sweep in the original algorithm on any real route.

---

## The Real Insight

The original algorithm is solving the wrong subproblem: *"find the best mapping for this stop given what came before."* That's greedy. The correct subproblem is: *"find the best mapping for all stops simultaneously subject to monotonicity."* That's DP.

The structure is identical to your bus arrival HMM — states are candidate projections, observations are stop coordinates, emission cost is distance, transition cost is 0 or ∞ based on progress ordering. Viterbi on that graph gives you the globally optimal monotone assignment in linear time.