# Preprocessor Integration Test Plan - Edge Cases (BDD Style)

**Version:** 1.0
**Date:** 2026-03-19
**Scope:** Integration test coverage for edge cases in the preprocessor module

---

## Test Philosophy

This plan uses **Behavior-Driven Development (BDD)** style with the **Given-When-Then** format:
- **Given**: Precondition / test setup
- **When**: Action / behavior under test
- **Then**: Expected outcome / assertion

---

## Table of Contents

1. [DP Mapper Edge Cases](#1-dp-mapper-edge-cases)
2. [Polyline Simplification Edge Cases](#2-polyline-simplification-edge-cases)
3. [Route Linearization Edge Cases](#3-route-linearization-edge-cases)
4. [Stop Protection Edge Cases](#4-stop-protection-edge-cases)
5. [Coordinate System Edge Cases](#5-coordinate-system-edge-cases)
6. [Grid Index Edge Cases](#6-grid-index-edge-cases)
7. [Binary Packing Edge Cases](#7-binary-packing-edge-cases)
8. [Cross-Module Integration Edge Cases](#8-cross-module-integration-edge-cases)

---

## 1. DP Mapper Edge Cases

### 1.1 Route Geometry Edge Cases

#### 1.1.1 U-Turn / Loop-Back Routes
**Scenario**: Route doubles back on itself creating overlapping segments

```gherkin
GIVEN a U-shaped route that goes east 100m, then north 50m, then west 100m
AND stops are placed at (50m, 0m) on east leg and (50m, 50m) on west leg
WHEN the DP mapper maps stops to route progress
THEN both stops should be mapped to different progress values
AND the west leg stop should have higher progress despite same X coordinate
AND monotonicity constraint (stop[i].progress <= stop[i+1].progress) is preserved
```

#### 1.1.2 Figure-8 Self-Crossing Route
**Scenario**: Route crosses itself at a point

```gherkin
GIVEN a figure-8 route that crosses itself at origin (0, 0)
AND two stops are both at the origin but on different loops
WHEN the DP mapper maps stops to route progress
THEN stops should map to different progress values based on visit order
AND monotonicity constraint is preserved across the crossing point
```

#### 1.1.3 Zig-Zag Route with Sharp Turns
**Scenario**: Route with frequent 90+ degree turns

```gherkin
GIVEN a route with alternating 90-degree turns every 10m (grid pattern)
AND stops are placed at each corner
WHEN the DP mapper maps stops to route progress
THEN all stops should be mapped correctly despite sharp heading changes
AND progress should increase monotonically through each turn
```

---

### 1.2 Stop Placement Edge Cases

#### 1.2.1 Stops at Segment Boundaries
**Scenario**: Stop exactly at segment node (t=0.0 or t=1.0)

```gherkin
GIVEN a route with segments of exactly 10m each
AND stops are placed exactly at segment boundaries (0m, 10m, 20m, 30m)
WHEN the DP mapper maps stops to route progress
THEN each stop should map to exact boundary progress values
AND no precision loss should occur at boundaries
```

#### 1.2.2 Stops 1cm from Segment Boundaries
**Scenario**: Numerical stability test near boundaries

```gherkin
GIVEN a route with segments at 0m, 10m, 20m
AND stops are placed at 9.99m, 10.01m, 19.99m
WHEN the DP mapper maps stops to route progress
THEN all stops should map correctly without boundary ambiguity
AND monotonicity should be preserved
```

#### 1.2.3 Identical Stops (Same Location)
**Scenario**: Multiple stops at identical coordinates

```gherkin
GIVEN a straight route
AND three stops are all at the exact same coordinates (50m, 0m)
WHEN the DP mapper maps stops to route progress
THEN all stops should return valid candidates
AND progress should be non-decreasing (may use snap-forward)
AND no candidate should have zero distance
```

#### 1.2.4 Stops Far from Route (100m+)
**Scenario**: Stop significantly off-route

```gherkin
GIVEN a straight route along X-axis from 0 to 100m
AND a stop is placed at (50m, 150m) - 150m perpendicular to route
WHEN the DP mapper maps the stop
THEN snap-forward mechanism should provide a valid candidate
AND the stop should still map to a valid progress value
AND the mapped position should be geometrically reasonable
```

#### 1.2.5 Dense Stops (More Stops Than Segments)
**Scenario**: 20 stops on a route with only 10 segments

```gherkin
GIVEN a 10m route with 10 segments (1m each)
AND 20 stops are placed every 0.5m
WHEN the DP mapper maps stops to route progress
THEN all 20 stops should return valid candidates
AND progress should be strictly monotonic (no duplicates allowed)
```

---

### 1.3 DP Algorithm Edge Cases

#### 1.3.1 Cost Saturation
**Scenario**: Accumulated costs approach i64::MAX

```gherkin
GIVEN a route with 100 stops
AND each stop candidate has distance squared of 10^12 cm²
WHEN the DP mapper computes optimal path
THEN costs should not overflow i64::MAX
AND a warning should be logged if saturation occurs
AND the result should still be valid
```

#### 1.3.2 Empty Candidate Set
**Scenario**: No candidates found for a stop

```gherkin
GIVEN a route with sparse segments
AND a stop is placed 1km away from any route segment
WHEN the DP mapper generates candidates for this stop
THEN the candidate set should be empty
AND map_stops_dp should return empty result
AND no panic should occur
```

#### 1.3.3 Single Candidate Per Stop
**Scenario**: K=1, only best candidate kept

```gherkin
GIVEN a complex route
AND K is set to 1 (only one candidate per stop)
WHEN the DP mapper maps stops
THEN each stop should have exactly one candidate
AND the global path should still be optimal with limited choice
```

#### 1.3.4 All Candidates Invalid (Monotonicity Violation)
**Scenario**: Current stop's candidates all have lower progress than previous stop

```gherkin
GIVEN a route
AND stop[0] maps to progress 1000m
AND stop[1] is geographically before stop[0] on the route
WHEN the DP mapper computes transitions
THEN snap-forward should provide a valid candidate for stop[1]
AND the final path should satisfy monotonicity constraint
```

---

### 1.4 Real-World Route Edge Cases

#### 1.4.1 Ty225 Real Route Validation
**Scenario**: Full 57-stop Taipei bus route

```gherkin
GIVEN the real ty225 route data with 57 stops
AND actual GPS coordinates from Taipei
WHEN the DP mapper maps all 57 stops
THEN all 57 stops should be mapped successfully
AND monotonicity should hold for all consecutive pairs
AND each stop's mapped position should be within 50m of actual GPS location
AND average mapping error should be < 20m
```

#### 1.4.2 Route with Heading Overflows
**Scenario**: Route segments with heading values near i16::MAX

```gherkin
GIVEN a route that circles multiple times
AND heading values exceed 360 degrees multiple times
WHEN the DP mapper processes the route
THEN heading calculations should handle wraparound correctly
AND no integer overflow should occur
```

---

## 2. Polyline Simplification Edge Cases

### 2.1 Douglas-Peucker Edge Cases

#### 2.1.1 Empty Route
```gherkin
GIVEN an empty route (no points)
WHEN Douglas-Peucker simplification is applied
THEN result should be empty
AND no error should occur
```

#### 2.1.2 Single Point Route
```gherkin
GIVEN a route with only one point
WHEN Douglas-Peucker simplification is applied
THEN result should contain exactly one point
```

#### 2.1.3 Two Point Route
```gherkin
GIVEN a route with exactly two points
WHEN Douglas-Peucker simplification is applied
THEN result should contain both points (line segment)
```

#### 2.1.4 Collinear Points
```gherkin
GIVEN a route with 100 points all on a straight line
WHEN Douglas-Peucker simplification is applied with epsilon=7m
THEN result should contain only 2 points (start and end)
AND intermediate points should be removed
```

#### 2.1.5 Epsilon Zero (No Simplification)
```gherkin
GIVEN a route with 1000 points
AND epsilon is set to 0
WHEN Douglas-Peucker simplification is applied
THEN result should contain all original points
AND no points should be removed
```

#### 2.1.6 Very Large Epsilon
```gherkin
GIVEN a route with 1000 points spanning 10km
AND epsilon is set to 1000m
WHEN Douglas-Peucker simplification is applied
THEN result should contain minimal points (likely 2-3)
```

---

### 2.2 Stop Protection Edge Cases

#### 2.2.1 Stop at Route Start
```gherkin
GIVEN a route from (0,0) to (1000,0)
AND a stop is placed exactly at route start (0,0)
WHEN simplification with stop protection is applied
THEN the start point should always be preserved
```

#### 2.2.2 Stop at Route End
```gherkin
GIVEN a route from (0,0) to (1000,0)
AND a stop is placed exactly at route end (1000,0)
WHEN simplification with stop protection is applied
THEN the end point should always be preserved
```

#### 2.2.3 Stop with 30m Radius Protection
```gherkin
GIVEN a route with points every 5m
AND a stop at 50m with 30m protection radius
WHEN simplification is applied
THEN all points within 30m of the stop should be preserved
AND approximately 12-13 points should remain in the protected zone
```

#### 2.2.4 Overlapping Protection Zones
```gherkin
GIVEN a route with two stops 40m apart
AND each has 30m protection radius
WHEN simplification is applied
THEN the overlapping region should still be protected
AND more points should be preserved than with a single stop
```

#### 2.2.5 Isolated Stop (No Nearby Points)
```gherkin
GIVEN a straight 1km route with points every 100m
AND a stop at 500m but 100m perpendicular to route
WHEN simplification with "guaranteed closest" rule is applied
THEN at least 3 points should be preserved (start, end, anchor)
AND the stop should have a valid projection
```

---

### 2.3 Sharp Turn Protection

#### 2.3.1 90-Degree Turn
```gherkin
GIVEN a route with a 90-degree turn
AND the turn has radius < 50m
WHEN simplification with curve protection is applied
THEN points defining the turn should be preserved with epsilon=2-3m
AND the turn geometry should not be distorted
```

#### 2.3.2 180-Degree U-Turn
```gherkin
GIVEN a route with a 180-degree U-turn
WHEN simplification with curve protection is applied
THEN all points defining the U-turn should be preserved
AND the turn should not collapse to a straight line
```

#### 2.3.3 Hairpin Turn (Series of Sharp Turns)
```gherkin
GIVEN a mountain road with multiple hairpin turns
WHEN simplification with curve protection is applied
THEN each hairpin turn should maintain its geometry
AND the route should not become self-intersecting
```

---

### 2.4 Maximum Segment Length Constraint

#### 2.4.1 Segment Exceeds 30m After Simplification
```gherkin
GIVEN a route simplified to have a 50m segment
WHEN max segment constraint (30m) is applied
THEN the segment should be split with intermediate points
AND no segment should exceed 30m
```

#### 2.4.2 Very Long Route Segment
```gherkin
GIVEN a route with a 1km straight section
WHEN simplification and max segment constraint are applied
THEN the 1km section should be split into ~33 segments of ~30m each
```

---

## 3. Route Linearization Edge Cases

### 3.1 Coordinate Conversion Edge Cases

#### 3.1.1 Equator Crossing
```gherkin
GIVEN a route that crosses the equator
WHEN coordinates are converted to cm
THEN the conversion should handle latitude = 0 correctly
AND no division by zero should occur
```

#### 3.1.2 Date Line Crossing
```gherkin
GIVEN a route that crosses the International Date Line
WHEN coordinates are converted to cm
THEN the conversion should handle longitude wraparound correctly
```

#### 3.1.3 Extreme Latitudes (Near Poles)
```gherkin
GIVEN a route at latitude 85 degrees North
WHEN coordinates are converted to cm using cos(lat_avg)
THEN the cos(lat_avg) should be very small but non-zero
AND x-coordinates should be compressed correctly
```

#### 3.1.4 Large Coordinate Values
```gherkin
GIVEN coordinates that exceed 32-bit integer range when converted
WHEN converted to cm
THEN i64 should be used for intermediate calculations
AND no overflow should occur
```

---

### 3.2 Distance Calculation Edge Cases

#### 3.2.1 Zero-Length Segment
```gherkin
GIVEN a route with two consecutive points at the same location
WHEN segment length is calculated
THEN the length should be exactly 0
AND the segment should be handled correctly (skipped or flagged)
```

#### 3.2.2 Very Small Segment (< 1cm)
```gherkin
GIVEN a route with two points 0.5cm apart
WHEN segment length is calculated
AND result is stored as i32 cm
THEN the length should round to 1cm or 0cm appropriately
```

#### 3.2.3 Point Distance Overflow
```gherkin
GIVEN two points very far apart (different continents)
WHEN squared distance is calculated as i64
THEN the calculation should not overflow
AND the correct distance should be returned
```

---

### 3.3 Cumulative Distance Edge Cases

#### 3.3.1 Accumulated Distance Overflow
```gherkin
GIVEN a very long route (> 21,000 km)
WHEN cumulative distance is calculated as i32 cm
THEN the distance should not overflow i32::MAX
AND appropriate error handling or type promotion should occur
```

#### 3.3.2 Floating Point Accumulation Error
```gherkin
GIVEN a route with 10,000 segments
WHEN cumulative distance is calculated by summing segment lengths
THEN floating point error should not exceed 1cm
AND integer arithmetic should be used where possible
```

---

## 4. Stop Protection Edge Cases

### 4.1 Stop Corridor Edge Cases

#### 4.1.1 First Stop Corridor (Negative Start)
```gherkin
GIVEN the first stop is at progress 5000cm (50m)
AND corridor is defined as 80m pre, 40m post
WHEN corridor boundaries are calculated
THEN corridor_start should be -8000cm (negative)
AND the negative value should be handled correctly
```

#### 4.1.2 Last Stop Corridor (Beyond Route End)
```gherkin
GIVEN the last stop is at progress 95000cm on a 100m route
AND corridor is defined as 80m pre, 40m post
WHEN corridor boundaries are calculated
THEN corridor_end should be 99000cm (beyond route end)
AND the exceeding boundary should be handled correctly
```

#### 4.1.3 Overlapping Corridors
```gherkin
GIVEN two consecutive stops only 50m apart
AND each has 80m pre / 40m post corridor
WHEN corridors are calculated
THEN the corridors should overlap significantly
AND overlap should be handled correctly in arrival detection
```

---

## 5. Coordinate System Edge Cases

### 5.1 Integer Type Conversions

#### 5.1.1 i64 to i32 Conversion
```gherkin
GIVEN a coordinate value in i64 that exceeds i32::MAX
WHEN converting to i32 for storage
THEN the value should be clamped or an error should be raised
AND no silent overflow should occur
```

#### 5.1.2 Floating Point to Integer Rounding
```gherkin
GIVEN a coordinate value of 123.6 cm
WHEN converting to integer cm
THEN the value should round to 124 cm
AND consistent rounding strategy should be used
```

---

### 5.2 Heading Calculations

#### 5.2.1 Heading Wraparound (360°)
```gherkin
GIVEN a heading of 359 degrees
AND a turn of +5 degrees
WHEN the new heading is calculated
THEN the result should be 4 degrees (not 364)
```

#### 5.2.2 Heading Difference Across 180°
```gherkin
GIVEN two headings: 10 degrees and 350 degrees
WHEN the difference is calculated
THEN the result should be 20 degrees (shortest path)
AND not 340 degrees
```

#### 5.2.3 Heading Overflow (i16::MAX)
```gherkin
GIVEN heading is stored as i16 centidegrees (0.01°)
AND a heading value of 18000 centidegrees (180°)
WHEN operations are performed
THEN no overflow should occur
AND values should stay within i16 range
```

---

## 6. Grid Index Edge Cases

### 6.1 Grid Construction

#### 6.1.1 Empty Route Grid
```gherkin
GIVEN an empty route (no segments)
WHEN a spatial grid is built
THEN the grid should have 0 columns and 0 rows
```

#### 6.1.2 Single Segment Grid
```gherkin
GIVEN a route with a single segment
WHEN a spatial grid is built
THEN the grid should have at least 1 cell
AND the segment should be indexed correctly
```

#### 6.1.3 Route Crossing Grid Boundaries
```gherkin
GIVEN a route with a 50m segment
AND grid cell size is 10m
WHEN the grid is built
THEN the segment should appear in multiple cells
AND all cells should be indexed correctly
```

---

### 6.2 Grid Query

#### 6.1.1 Query Outside Grid Bounds
```gherkin
GIVEN a grid covering x: [0, 1000], y: [0, 1000]
AND a query at (2000, 2000)
WHEN neighbors are queried
THEN the result should be empty
AND no out-of-bounds access should occur
```

#### 6.1.2 Query with Radius Larger Than Grid
```gherkin
GIVEN a 3x3 grid
AND a query with radius 10
WHEN neighbors are queried
THEN only valid cells should be returned
AND no out-of-bounds access should occur
```

#### 6.1.3 Duplicate Segment in Multiple Cells
```gherkin
GIVEN a segment that spans multiple grid cells
WHEN neighbors are queried
THEN the segment should only appear once in results
AND deduplication should work correctly
```

---

## 7. Binary Packing Edge Cases

### 7.1 Data Serialization

#### 7.1.1 Empty Route Data
```gherkin
GIVEN an empty route (no nodes, no stops)
WHEN the route is serialized to binary
THEN the binary should be valid
AND it should deserialize correctly
```

#### 7.1.2 Maximum Size Route
```gherkin
GIVEN a route with maximum practical nodes (1000+)
WHEN the route is serialized to binary
THEN the binary size should be within Flash limits (~34KB)
AND deserialization should succeed
```

#### 7.1.3 Node Alignment
```gherkin
GIVEN route nodes with various fields
WHEN serializing to binary
THEN each node should be properly aligned
AND no padding should cause incorrect deserialization
```

---

### 7.2 Data Deserialization

#### 7.2.1 Corrupted Binary Data
```gherkin
GIVEN corrupted binary route data
WHEN deserialization is attempted
THEN the operation should fail gracefully
AND a clear error should be returned
```

#### 7.2.2 Version Mismatch
```gherkin
GIVEN binary data from a different format version
WHEN deserialization is attempted
THEN the version mismatch should be detected
AND an appropriate error should be returned
```

---

## 8. Cross-Module Integration Edge Cases

### 8.1 Full Pipeline Integration

#### 8.1.1 Real-World Route with All Edge Cases
```gherkin
GIVEN a real bus route (ty225) with 57 stops
AND the route contains sharp turns, close stops, and U-turns
WHEN the full preprocessor pipeline runs
THEN all stops should be mapped correctly
AND binary output should be valid
AND Flash size should be within limits
```

#### 8.1.2 Minimal Valid Route
```gherkin
GIVEN a route with just 2 points and 1 stop
WHEN the full preprocessor pipeline runs
THEN a valid binary should be produced
AND all modules should handle minimal input correctly
```

#### 8.1.3 Maximum Load Route
```gherkin
GIVEN a route with 1000 points and 100 stops
WHEN the full preprocessor pipeline runs
THEN processing should complete in reasonable time
AND memory usage should stay within bounds
```

---

### 8.2 Error Propagation

#### 8.2.1 Invalid Input Data
```gherkin
GIVEN route JSON with missing required fields
WHEN the preprocessor processes the input
THEN a clear error message should be produced
AND the error should indicate which field is missing
```

#### 8.2.2 Invalid GPS Coordinates
```gherkin
GIVEN a stop with latitude > 90 or longitude > 180
WHEN the preprocessor processes the input
THEN a validation error should be raised
AND the invalid coordinate should be reported
```

---

## Test Implementation Priority

### High Priority (Must Have)
1. DP Mapper: Route loops, U-turns, dense stops
2. DP Mapper: Ty225 real-world validation
3. Stop Protection: 30m radius, overlapping zones
4. Coordinate System: Heading wraparound, large values

### Medium Priority (Should Have)
1. Polyline Simplification: Sharp turns, max segment length
2. Grid Index: Boundary queries, multi-cell segments
3. Route Linearization: Distance calculation overflow

### Low Priority (Nice to Have)
1. Binary Packing: Corrupted data handling
2. Extreme Latitudes: Near-pole routes
3. Date Line Crossing

---

## Success Criteria

Each edge case test should:
1. Follow the Given-When-Then structure
2. Be reproducible (deterministic)
3. Have clear assertions
4. Include error message strings for debugging
5. Document the edge case being tested
