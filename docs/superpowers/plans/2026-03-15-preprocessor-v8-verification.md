# Preprocessor QA Plan: Tech Report v8 Verification

## Objective
Verify that the `preprocessor` module correctly implements the algorithms and constraints defined in `docs/bus_arrival_tech_report_v8.md`.

## Gap Analysis
Based on a code review of `preprocessor/src/` against `bus_arrival_tech_report_v8.md`, the implementation appears to match the specifications. However, several critical logic branches lack explicit integration tests:

1.  **Sharp Curve Protection (Module ①):** The logic for reducing $\varepsilon$ to 2.5m for turns > 20° exists in `simplify.rs`, but no test specifically exercises a route with such curvature to ensure points are preserved.
2.  **Stop Corridor Overlap (Module ⑨):** The logic for truncating corridors with 20m separation exists in `stops.rs`, but no test simulates stops closer than $(L_{pre} + L_{post} + 20m) \approx 140m$ to verify the truncation.
3.  **Max Segment Length (Module ①/②):** The logic to interpolate segments > 30m exists in `simplify.rs`, but no test verifies this behavior on a long straight route.
4.  **Spatial Grid Indexing (Module ③):** No tests verify that segments are correctly mapped to multiple grid cells when crossing boundaries.
5.  **Linearization Coefficients (Module ②):** No test verifies the calculation of `line_a`, `line_b`, `line_c`, and `heading_cdeg` against known geometric ground truth.

## Proposed Tests

Create a new integration test file `preprocessor/tests/tech_report_v8_verification.rs` containing the following scenarios:

### 1. Test Sharp Curve Preservation
*   **Goal:** Verify Module ① curve protection.
*   **Scenario:** Create a "V" shaped route with a 90° turn.
    *   Point A: (0, 0)
    *   Point B: (10m, 0)
    *   Point C: (10m, 10m)
    *   Points are dense (e.g., every 1m).
*   **Expectation:** Even with $\varepsilon = 7m$ (which would normally simplify A->C to a straight line), the sharp corner at B (and surrounding points) must be preserved because the turn angle > 20°.

### 2. Test Stop Corridor Overlap
*   **Goal:** Verify Module ⑨ overlap truncation.
*   **Scenario:** Create a straight route with two stops separated by 50m.
    *   Stop 1 at 100m.
    *   Stop 2 at 150m.
*   **Expectation:**
    *   Stop 1 Corridor: [20m, 140m] (Normal: 100-80 to 100+40)
    *   Stop 2 Corridor Start: Must be $\ge$ Stop 1 End + 20m = 160m.
    *   Original Stop 2 Start would be 150 - 80 = 70m.
    *   Verification: `stop2.corridor_start_cm` should be 16000 (160m), not 7000.

### 3. Test Max Segment Length
*   **Goal:** Verify Module ①/② interpolation.
*   **Scenario:** Create a straight route with two points 100m apart.
*   **Expectation:** The final binary should contain intermediate nodes such that no segment exceeds 30m.
    *   `route_data.node_count` should be $\ge 4$ (Start + 2 intermediate + End).
    *   Max `seg_len_cm` in `route_data` should be $\le 3000$.

### 4. Test Spatial Grid Construction
*   **Goal:** Verify Module ③ grid indexing.
*   **Scenario:** Create a route that spans multiple 100m x 100m grid cells.
    *   Point A: (50m, 50m) -> Cell (0,0)
    *   Point B: (150m, 50m) -> Cell (1,0)
*   **Expectation:**
    *   Grid dimensions should be at least 2x1.
    *   The segment A->B should be indexed in both Cell (0,0) and Cell (1,0).

### 5. Test Linearization Coefficients
*   **Goal:** Verify Module ② geometric calculations.
*   **Scenario:** Create a simple 3-4-5 triangle route segment.
    *   Start: (0, 0)
    *   End: (30m, 40m) -> Length 50m.
*   **Expectation:**
    *   `dx_cm` = 3000
    *   `dy_cm` = 4000
    *   `len2_cm2` = 25,000,000
    *   `line_a` = -4000
    *   `line_b` = 3000
    *   `heading_cdeg` matches `atan2(40, 30)` (~53.13°).

## Verification Plan
1.  Implement the tests in `preprocessor/tests/tech_report_v8_verification.rs`.
2.  Run `cargo test -p preprocessor --test tech_report_v8_verification`.
3.  Ensure all assertions pass.
