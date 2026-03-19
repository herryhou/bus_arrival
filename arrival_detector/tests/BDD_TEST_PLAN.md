# Arrival Detector BDD Test Plan

This document outlines the behavior-driven development (BDD) test scenarios for the `arrival_detector` crate, covering arrival detection logic, state machine transitions, and edge cases.

## 1. Stop Corridor Filtering

### Scenario: Filtering active stops by route progress
*   **Given** a route has three stops at 1km, 5km, and 10km
*   **And** each stop has a corridor of [-80m, +40m]
*   **When** the bus's route progress is at 1010m (within the 1st stop's corridor)
*   **Then** only the 1st stop should be identified as "active"
*   **And** the other stops should be ignored

### Scenario: Handling overlapping corridors
*   **Given** two stops are only 50m apart
*   **And** their corridors overlap (Stop A: [920m, 1040m], Stop B: [970m, 1090m])
*   **When** the bus is at 1000m
*   **Then** both Stop A and Stop B should be identified as "active"

## 2. Bayesian Arrival Probability

### Scenario: High probability at a full stop
*   **Given** the bus is 5 meters from the stop (Progress Diff ≈ 0)
*   **And** the bus speed is 0 cm/s
*   **And** the bus has been stationary for 15 seconds (Dwell Time > 10s)
*   **When** the arrival probability is calculated
*   **Then** the probability should be very high (exceeding the 75% threshold)

### Scenario: Low probability for high-speed pass-by (Skip-Stop Protection)
*   **Given** the bus is 2 meters from the stop
*   **But** the bus speed is 40 km/h (1111 cm/s)
*   **And** the dwell time is only 1 second
*   **When** the arrival probability is calculated
*   **Then** the probability should be low (below the 75% threshold)
*   **And** no arrival should be triggered

### Scenario: Low probability when far from stop
*   **Given** the bus is stationary (0 cm/s)
*   **But** the bus is 70 meters away from the stop (near the corridor boundary)
*   **When** the arrival probability is calculated
*   **Then** the distance-based likelihood should significantly reduce the total probability
*   **And** remain below the 75% threshold

## 3. Stop State Machine (FSM)

### Scenario: Standard arrival sequence
*   **Given** a stop is in the `Approaching` state
*   **When** the bus gets closer than 50m to the stop
*   **Then** the state should transition to `Arriving`
*   **When** the arrival probability then exceeds 75%
*   **Then** the state should transition to `AtStop`
*   **And** an `ArrivalEvent` should be emitted exactly once

### Scenario: Premature departure (False Arrival Prevention)
*   **Given** a stop is in the `Arriving` state (within 50m)
*   **When** the bus moves past the stop (d > 40m AND s > stop_progress) without the probability ever exceeding 75%
*   **Then** the state should transition directly to `Departed`
*   **And** no arrival should be recorded

### Scenario: Stop re-activation (U-Turn / Loop handling)
*   **Given** a stop is in the `Departed` state
*   **When** the bus moves back into the stop's corridor (e.g., after a loop or U-turn)
*   **Then** the state machine should reset back to `Approaching`
*   **And** be ready to detect a second arrival at the same stop

## 4. GPS Jump & Recovery

### Scenario: Forward jump to next stop
*   **Given** the last detected stop was Index 0
*   **When** a GPS jump of 500m occurs, placing the bus near Stop Index 2
*   **Then** the recovery logic should identify Stop Index 2 as the new current stop
*   **And** intervening stops (Index 1) should be marked as skipped or reset

### Scenario: Backward jump penalty (Stability)
*   **Given** the bus is near Stop Index 5
*   **When** GPS noise causes a small backward jump toward Stop Index 4
*   **Then** the index penalty in the recovery scoring should prevent "jumping back" unless the backward movement is very large and persistent

## 5. Input & Trace Verification

### Scenario: Heading preservation from localization
*   **Given** a Phase 2 input record with a heading of -950 (350.5°)
*   **When** the record is parsed by the arrival detector
*   **Then** the heading value should be preserved in the `InputRecord`
*   **And** appear correctly in the debug trace output

### Scenario: Detailed feature trace
*   **Given** the bus is at a stop
*   **When** a trace record is emitted
*   **Then** it should contain individual scores for all 4 features (Distance, Speed, Progress, Dwell Time)
*   **And** the FSM state should be correctly serialized as a string (e.g., "AtStop")
