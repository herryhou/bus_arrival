# Simulator BDD Test Plan

This document outlines the behavior-driven development (BDD) test scenarios for the `simulator` crate, covering core localization, map matching, edge cases, and arrival detection.

## 1. Core Localization & Smoothing

### Scenario: Normal forward movement on a straight road
*   **Given** a bus is moving on a straight 1km route
*   **And** the GPS signal is accurate (HDOP = 1.0)
*   **When** the bus moves at a constant speed of 10m/s
*   **Then** the estimated route progress should match the physical distance traveled
*   **And** the Kalman filter should provide a smooth trajectory

### Scenario: Adaptive smoothing for noisy GPS
*   **Given** a bus is moving at 10m/s
*   **When** a GPS update has high noise (HDOP = 5.0)
*   **Then** the Kalman filter should give less weight to the GPS update
*   **And** the route progress should stay closer to the predicted position than the raw GPS point

### Scenario: Handling sharp turns
*   **Given** the route has a 90-degree turn
*   **When** the bus enters the turn at 5m/s
*   **Then** the map matcher should correctly identify the next segment based on the change in heading
*   **And** the route progress should remain continuous through the turn

## 2. Map Matching & Segment Selection

### Scenario: Heading-constrained segment selection (Overlapping Routes)
*   **Given** two route segments are physically close but have opposite headings (e.g., a divided highway)
*   **And** the bus is moving on the north-bound segment
*   **When** a GPS point is received that is physically closer to the south-bound segment
*   **But** the GPS heading matches the north-bound segment
*   **Then** the map matcher should select the north-bound segment
*   **And** ignore the south-bound segment despite the closer distance

### Scenario: Recovery via Grid Search
*   **Given** the bus was lost or had a large GPS jump
*   **When** a new valid GPS fix is received far from the last known position
*   **Then** the map matcher should fall back to a global grid search
*   **And** successfully snap back to the correct part of the route

## 3. GPS Signal Edge Cases

### Scenario: GPS Jump Rejection
*   **Given** the bus is at progress 100m
*   **When** a GPS update suddenly reports a position 500m away in 1 second
*   **Then** the simulator should reject the update as a "speed constraint" violation
*   **And** maintain the previous valid state

### Scenario: GPS Outage & Dead Reckoning (Short term)
*   **Given** the bus is moving at 10m/s
*   **When** the GPS signal is lost for 3 seconds
*   **Then** the simulator should enter Dead Reckoning (DR) mode
*   **And** estimate the progress based on the last known speed (approx. +30m)

### Scenario: Extended GPS Outage (Long term)
*   **Given** the bus is in Dead Reckoning mode
*   **When** the GPS signal remains lost for more than 10 seconds
*   **Then** the simulator should stop reporting progress
*   **And** return an `Outage` status

## 4. Physical & Routing Constraints

### Scenario: Maximum Speed Limit
*   **Given** the maximum allowed speed is 108 km/h (3000 cm/s)
*   **When** GPS updates imply a speed of 150 km/h
*   **Then** those updates should be rejected
*   **And** the state should not be updated to the impossible position

### Scenario: Monotonicity with Noise Tolerance
*   **Given** the bus is at progress 500m
*   **When** GPS noise suggests a position at 495m (5m backward)
*   **Then** the update should be accepted (within the 500m noise tolerance)
*   **But** a jump back to 100m should be rejected if it's outside the tolerance

### Scenario: Route Reversal Detection
*   **Given** the bus is on a one-way route
*   **When** the bus starts moving in the opposite direction of the route segments
*   **Then** the map matcher's heading penalty should increase
*   **And** eventually, the updates should be rejected if they deviate too far from the route's directed path

## 5. Route Boundary Conditions

### Scenario: Initial Fix at Route Start
*   **Given** a new trip is starting
*   **When** the first GPS fix is received near the first node of the route
*   **Then** the simulator should initialize the state to the beginning of the route
*   **And** set the initial progress to 0 (or the exact projection on the first segment)

### Scenario: Reaching the End of Route
*   **Given** the bus is near the last node of the route
*   **When** it moves past the last node
*   **Then** the progress should clamp to the total route length
*   **And** subsequent updates should indicate the end of the trip

### Scenario: Loop Closure
*   **Given** a circular route where the start and end nodes are at the same location
*   **When** the bus completes a full lap
*   **Then** the map matcher should distinguish between being at the "start" (idx 0) and the "end" (idx N) based on previous progress and heading
*   **And** avoid "jumping" back to the start prematurely

## 6. Arrival Detection (Active Stops)

### Scenario: Entering a Stop Corridor
*   **Given** a stop is defined at 1000m with a corridor from 800m to 1200m
*   **When** the bus's estimated progress reaches 850m
*   **Then** the stop should be identified as "active"
*   **And** the output should include the stop index in the active list

### Scenario: Overlapping Stop Corridors
*   **Given** two stops are very close to each other
*   **And** their corridors overlap
*   **When** the bus is in the overlapping region
*   **Then** both stops should be reported as active simultaneously

### Scenario: Skipping a Stop
*   **Given** a bus is moving very fast or GPS skips an area
*   **When** the bus's progress jumps from 700m to 1300m (skipping the 800-1200m corridor)
*   **Then** the stop should NOT be reported as active (as it was never "in" the corridor at a sample point)
*   *Note: This is current behavior; a future enhancement might interpolate to catch missed stops.*
