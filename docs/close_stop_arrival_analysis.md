# COMPREHENSIVE ANALYSIS: Stop #2 → Stop #3 (79.32m apart)        
                                                                                                                                      
## 1. GEOMETRIC CONFIGURATION
                                                                                                                                      
  Stop #2:  progress_cm = 127,689 cm                              
  Stop #3:  progress_cm = 135,621 cm
  Distance:              7,932 cm (79.32m)

```txt
  Corridor Configuration:
  ├── corridor_start_cm = progress_cm - 8000  (80m before stop)
  ├── corridor_end_cm   = progress_cm + 4000  (40m after stop)                                                                        
  └── Total corridor length = 120m
```                                                                                                                                      
  Stop #2 corridor: [119,689 ~ 131,689]                           
  Stop #3 corridor: [127,621 ~ 139,621]

  *** OVERLAP: 4,068 cm (40.7m) ***

  Corridor Visualization
```txt
  Position (cm):  119k    122k    125k    128k    131k    134k    137k    140k
                 |       |       |       |       |       |       |       |                                                            
  Stop #2:       |-------[=======STOP2=======]-------|
  Stop #3:               |-------[=======STOP3=======]-------|                                                                        
                 |       |       |       |       |       |       |       |
                 ↑       ↑       ↑       ↑       ↑       ↑       ↑       ↑
              119689  122689  125689  128689  131689  134689  137689  140689
                        └───────────────────────┘                                                                                     
                           OVERLAP REGION
                        (40.7m = 4068cm)
```                                  
                                                                  
  ---
##  2. FSM STATE MACHINE RULES

  State Transitions
  ```rust
  // From state_machine.rs

  Idle → Approaching:     s_cm >= corridor_start_cm
  Approaching → Arriving: d_to_stop < 5000 (50m)
  Arriving → AtStop:      d_to_stop < 5000 && probability > 191
  Arriving → Departed:    d_to_stop > 4000 && s_cm > stop_progress
  AtStop → Departed:      d_to_stop > 4000 && s_cm > stop_progress                                                                    
   
  Active Stop Filter (corridor.rs)                                                                                                    
                                                        
  pub fn find_active_stops(s_cm, stops) {
      stops.filter(|stop|
          s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
      )
  }
```
  KEY INSIGHT: Multiple stops CAN be active simultaneously when corridors overlap!

  ---
##  3. PROBABILITY FORMULA

  // From probability.rs
```rust
  // Feature calculations:
  p1 = gaussian(distance_cm, sigma=2750)     // Distance likelihood
  p2 = logistic(speed_cm/s, v_stop=200)      // Speed likelihood (near 0 = high)                                                      
  p3 = gaussian(distance_cm, sigma=2000)     // Progress difference
  p4 = min(dwell_time_s * 255 / 10, 255)     // Dwell time (T_ref = 10s)                                                              
                                                                  
  // Weighted sum:
  probability = (13*p1 + 6*p2 + 10*p3 + 3*p4) / 32
```
  THETA_ARRIVAL = 191  // Threshold for AtStop transition                                                                             
   
  Feature Weights                                                                                                                     
                                                                  
  - p1: 40.6% - Most important (distance)
  - p2: 18.8% - Speed penalty
  - p3: 31.2% - Progress alignment
  - p4: 9.4% - Dwell time

  ---
##  4. DETAILED EVENT TIMELINE

  TIME | s_cm (position) | Stop #2 State | Dist to #2 | Stop #3 State | Dist to #3 | Notes
  -----|------------------|---------------|------------|---------------|------------|-------
   451 | 121,489         | Approaching   | -6,619     | -             | -          | Enter #2 corridor                                
   455 | 124,453         | Arriving      | -4,715     | -             | -          | Within 50m of #2
   461 | 127,689         | Arriving      | -1,804     | -             | -          | Approaching #2                                   
   463 | 128,790         | Arriving      | -1,052     | -             | -          |
   465 | 129,889         | AtStop ✓      |   -200     | -             | -          | **ARRIVAL #2**                                   
   467 | 131,028         | AtStop        |    757     | -             | -          | Past #2, still AtStop
   469 | 132,323         | AtStop        |  1,692     | -             | -          |                                                  
   471 | 133,433         | AtStop        |  2,583     | -             | -          | *** KEY MOMENT ***
   473 | 134,571         | AtStop        |  3,514     | -             | -          | Still < 4000cm                                   
   475 | 135,883         | Departed      | -          | -             | -          | #2 departed
   477 | 137,114         | -             | -          | -             | -          | In overlap zone                                  
   479 | 138,219         | -             | -          | -             | -          | In overlap zone
   481 | 139,258         | -             | -          | Arriving      |   -907     | #3 activates!
   483 | 140,609         | -             | -          | Arriving      |     -7     | Closest to #3                                    
   485 | 141,837         | -             | -          | Arriving      |    875     |
                                                                                                                                      
  ---                                                             
##  5. THE PROBLEM: Stop #3 Misses Arrival

  At time 483 (closest approach to Stop #3)

  Time: 483
  Position: 140,609 cm                                                                                                                
  Speed: 783 cm/s (28 km/h)
                                                                                                                                      
  Stop #3 State:                                                  
  ├── FSM: Arriving (NOT AtStop!)
  ├── Distance: -7cm (practically AT the stop)
  ├── Probability: 185 (below threshold of 191)
  └── Features:
      ├── p1 = 255 (perfect distance)
      ├── p2 = 0   (high speed penalty)
      ├── p3 = 255 (perfect progress)
      └── p4 = 25  (dwell time only 1s)

  Probability Calculation:                                                                                                            
  = (13*255 + 6*0 + 10*255 + 3*25) / 32
  = (3315 + 0 + 2550 + 75) / 32                                                                                                       
  = 5940 / 32                                                     
  = 185.625 ≈ 185 ✗ < 191

  Why is p4 only 25?

  At time 481: Stop #3 FIRST activates
  ├── dwell_time_s = 1 (just entered corridor!)
  ├── p4 = (1 * 255) / 10 = 25.5 ≈ 25
  └── This is the FIRST tick in Arriving state                                                                                        
  
  At time 483: SECOND tick                                                                                                            
  ├── dwell_time_s = 2                                            
  ├── p4 = (2 * 255) / 10 = 51
  └── Still too low to compensate for p2=0

  ---
##  6. COMPARISON: Stop #2 Succeeded, Stop #3 Failed

  ┌───────────────┬────────────────────┬────────────────────┬──────────────────┐
  │   Parameter   │ Stop #2 (time 465) │ Stop #3 (time 483) │    Why #2 Won    │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤                                                      
  │ Distance      │ -200cm             │ -7cm               │ Both excellent   │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤                                                      
  │ Speed         │ 762 cm/s           │ 783 cm/s           │ Similar          │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤
  │ p1 (distance) │ 254                │ 255                │ Both perfect     │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤
  │ p2 (speed)    │ 0                  │ 0                  │ Both penalized   │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤
  │ p3 (progress) │ 253                │ 255                │ Both perfect     │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤
  │ p4 (dwell)    │ 204                │ 25                 │ KEY DIFFERENCE   │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤
  │ dwell_time_s  │ 9s                 │ 1s                 │ #2 had more time │
  ├───────────────┼────────────────────┼────────────────────┼──────────────────┤
  │ Probability   │ 201 ✓              │ 185 ✗              │ p4 decides       │
  └───────────────┴────────────────────┴────────────────────┴──────────────────┘                                                      
  
  Why did Stop #2 have higher dwell time?                                                                                             
                                                                  
  Stop #2 Timeline:
  ├── time 447: Enters corridor (Approaching)
  ├── time 449: Still Approaching
  ├── time 451: Approaching, dwell=2s
  ├── time 453: Approaching, dwell=3s
  ├── time 455: Arriving, dwell=4s
  ├── time 463: Arriving, dwell=8s
  └── time 465: AtStop! dwell=9s, p4=204
                                                                                                                                      
  Stop #3 Timeline:
  ├── time 473: Stop #2 still AtStop (blocks #3)                                                                                      
  ├── time 475: Stop #2 departs                                   
  ├── time 475-479: NO active stops (gap!)
  ├── time 481: Stop #3 activates, dwell=1s
  └── time 483: Arriving, dwell=2s, p4=25

  ---
##  7. ROOT CAUSE ANALYSIS
                        
  Issue 1: Stop #2 Blocks Stop #3 Activation
                                                                                                                                      
  At time 471 (bus at 133,433 cm):
  ├── Position relative to Stop #3: 135,621 - 133,433 = 2,188cm BEFORE                                                                
  ├── Stop #3 corridor starts at: 135,621 - 8,000 = 127,621 cm    
  ├── Bus IS in Stop #3's corridor: 133,433 > 127,621 ✓
  ├── But Stop #2 is STILL in AtStop state
  └── find_active_stops() SHOULD return both [2, 3]
                                                                                                                                      
  QUESTION: Does the system allow multiple active stops?
                                                                                                                                      
  Looking at the trace output:                                    
  - time 471: active_stops=[2] - Only Stop #2!
  - This means Stop #3 is NOT being filtered as active

  POSSIBLE BUG: There may be logic that prevents multiple stops from being active simultaneously.

  Issue 2: Late Activation = Low Dwell Time

  Stop #3 activates at time 481:
  ├── Bus position: 139,258 cm
  ├── Distance to Stop #3: 139,258 - 135,621 = 3,637 cm PAST
  ├── Already past the stop by 36m!                                                                                                   
  └── dwell_time_s starts at 1 (too low for p4 to compensate)
                                                                                                                                      
  Issue 3: High Speed Through Corridor                            

  Speed during Stop #3 corridor transit: 750-800 cm/s (27-28 km/h)
  ├── At this speed, p2 ≈ 0 (severe penalty)
  ├── Bus crosses entire 79m gap in ~10 seconds
  └── No time to accumulate dwell_time_s

  ---
##  8. DESIGN CONSTRAINTS & ASSUMPTION
                                                                                                                                      
  1. Corridor Configuration: 80m before, 40m after (total 120m)
  2. Stops at 79m apart: Significant corridor overlap expected                                                                        
  3. Single-activation assumption: Code may assume only one stop active at a time
  4. Dwell time feature: Designed for buses that STOP at stops, not fly-through
  5. Probability threshold: Fixed at 191 regardless of stop spacing

  ---
  9. EXPECTED BEHAVIOR (What Should Happen)

  For closely-spaced stops (79m apart):

  1. Both stops should be active simultaneously during overlap                                                                        
    - find_active_stops() should return [2, 3] when s_cm is in [127,621, 131,689]
  2. Stop #2 should depart earlier                                                                                                    
    - Current: AtStop → Departed when d_to_stop > 4000cm          
    - Issue: Bus is 3514cm past Stop #2, already in Stop #3's corridor
    - Should depart when leaving immediate stop area (e.g., 500-1000cm)
  3. Stop #3 should activate earlier
    - Current: Activates at time 481 (already 3637cm PAST)                                                                            
    - Should activate when entering corridor at time ~470
  4. Probability model should handle close stops                                                                                      
    - Current: p4 (dwell time) works against close-stop detection 
    - Should reduce p4 weight or use alternative model for close stops

---

## 10. PROPOSED SOLUTIONS

Based on the comprehensive analysis, here are multiple solution approaches:

### Solution 1: Allow Multiple Active Stops (Recommended)

#### Problem
Currently, the system appears to only process one stop at a time, even when `find_active_stops()` returns multiple indices.

#### Solution
Verify and ensure that multiple stops can be active simultaneously when corridors overlap.

#### Implementation Changes

**File: `arrival_detector/src/main.rs`**

The `find_active_stops()` function in `corridor.rs` already supports multiple stops:

```rust
pub fn find_active_stops(s_cm: DistCm, stops: &[Stop]) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(_, stop)| {
            s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
        })
        .map(|(i, _)| i)
        .collect()
}
```

**Verification needed**: Check if the main loop properly iterates through ALL active stops:

```rust
let active_indices = corridor::find_active_stops(record.s_cm, &stops);

for &stop_idx in &active_indices {
    // Process each stop
}
```

#### Expected Behavior After Fix
```
time=471: active_stops=[2,3]
├── Stop #2: AtStop (departing soon)
└── Stop #3: Approaching (activating early)
```

---

### Solution 2: Earlier Departure from AtStop State

#### Problem
Stop #2 stays in AtStop until distance > 4000cm (40m past stop).

#### Solution
Transition AtStop → Departed earlier, when the bus has clearly left the immediate stop area.

#### Implementation Changes

**File: `arrival_detector/src/state_machine.rs`**

```rust
// CURRENT (lines 92-97):
FsmState::AtStop => {
    if d_to_stop > 4000 && s_cm > stop_progress {
        self.fsm_state = FsmState::Departed;
    }
}

// PROPOSED: Add earlier departure for close stops
FsmState::AtStop => {
    // Check if next stop is close and we're in its corridor
    let should_depart_early = d_to_stop > 1500 && s_cm > stop_progress;

    if should_depart_early || (d_to_stop > 4000 && s_cm > stop_progress) {
        self.fsm_state = FsmState::Departed;
    }
}
```

#### Why 1500cm (15m)?
- Bus typically stops within 5-10m of the stop location
- 15m past the stop indicates clearly departed
- Still within corridor for overlapping stops
- Allows next stop to activate sooner

#### Alternative: Dynamic Departure Threshold

```rust
// Calculate distance to next stop
let dist_to_next = if self.index < MAX_STOPS {
    next_stop.progress_cm - stop_progress
} else {
    i32::MAX
};

// If next stop is close (<100m), depart earlier
let depart_threshold = if dist_to_next < 10000 {
    1500  // 15m for close stops
} else {
    4000  // 40m for normal stops
};

if d_to_stop > depart_threshold && s_cm > stop_progress {
    self.fsm_state = FsmState::Departed;
}
```

---

### Solution 3: Reduce p4 Weight for Close Stops

#### Problem
The dwell time feature (p4) penalizes close stops because the bus hasn't been in the corridor long enough.

#### Solution
Use adaptive probability weights based on stop spacing.

#### Implementation Changes

**File: `arrival_detector/src/probability.rs`**

```rust
/// Compute arrival probability with adaptive weights for close stops
pub fn arrival_probability_adaptive(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
    next_stop: Option<&shared::Stop>,
) -> Prob8 {
    // Standard feature calculation
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2] as u32;

    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3] as u32;

    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Detect if next stop is close (<100m)
    let is_close_stop = next_stop.map_or(false, |next| {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        dist_to_next < 10000  // 100m
    });

    // Use different weights for close stops
    let (w1, w2, w3, w4) = if is_close_stop {
        (15, 8, 12, 0)  // Remove p4 weight for close stops
    } else {
        (13, 6, 10, 3)  // Standard weights
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

#### Recalculated Probability for Stop #3

```
Standard weights: probability = 185 ✗
With p4=0 removed:
= (15*255 + 8*0 + 12*255 + 0*0) / 32
= (3825 + 0 + 3060 + 0) / 32
= 6885 / 32
= 215.1 ≈ 215 ✓ > 191
```

---

### Solution 4: Minimum Probability Boost for Close Stops

#### Problem
Even with perfect distance (p1=255, p3=255), high speed (p2=0) and low dwell (p4=25) prevent arrival detection.

#### Solution
Add a "closeness bonus" when stops are within 100m of each other.

#### Implementation Changes

**File: `arrival_detector/src/probability.rs`**

```rust
pub const THETA_ARRIVAL: Prob8 = 191;
pub const CLOSE_STOP_BONUS: Prob8 = 30;  // Bonus for closely-spaced stops
pub const CLOSE_STOP_THRESHOLD: DistCm = 10000;  // 100m

pub fn arrival_probability_with_bonus(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
    next_stop: Option<&shared::Stop>,
) -> Prob8 {
    // Standard probability calculation
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2] as u32;

    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3] as u32;

    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    let base_prob = (13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32;

    // Add closeness bonus for adjacent stops <100m
    let bonus = next_stop.map_or(0, |next| {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < CLOSE_STOP_THRESHOLD {
            CLOSE_STOP_BONUS
        } else {
            0
        }
    });

    (base_prob + bonus).min(255) as u8
}
```

#### Result for Stop #3

```
Base probability: 185
Closeness bonus: +30
Final: 215 ✓ > 191
```

---

### Solution 5: Corridor Entry Time Initialization

#### Problem
Stop #3's dwell_time starts at 1 when it first activates, even though the bus has been in the corridor for several seconds.

#### Solution
Initialize dwell_time based on how long the bus has been in the corridor.

#### Implementation Changes

**File: `arrival_detector/src/main.rs`**

Track corridor entry time for each stop:

```rust
// Add to StopState
pub struct StopState {
    pub index: u8,
    pub fsm_state: FsmState,
    pub dwell_time_s: u16,
    pub last_probability: Prob8,
    pub last_announced_stop: u8,
    pub corridor_entry_time: Option<u32>,  // NEW: Track when we entered corridor
}

// Modify update() to initialize dwell_time on corridor entry
FsmState::Idle => {
    if s_cm >= corridor_start_cm {
        self.fsm_state = FsmState::Approaching;
        // Calculate time already spent in corridor
        if let Some(entry_time) = self.corridor_entry_time {
            let time_in_corridor = current_time - entry_time;
            self.dwell_time_s = (time_in_corridor / 1000).min(10) as u16;
        }
    }
}
```

**Alternative: Use distance-based dwell initialization**

```rust
// Initialize dwell_time based on distance into corridor
FsmState::Idle => {
    if s_cm >= corridor_start_cm {
        self.fsm_state = FsmState::Approaching;
        // Estimate time in corridor based on distance
        let dist_into_corridor = s_cm - corridor_start_cm;
        let estimated_dwell = (dist_into_corridor / 1000).min(10) as u16;
        self.dwell_time_s = estimated_dwell;
    }
}
```

---

### Solution 6: Dual-Stop Processing Mode

#### Problem
The FSM processes stops sequentially, missing overlapping corridors.

#### Solution
Add a "close stop pair" mode that processes both stops together.

#### Implementation Changes

**File: `arrival_detector/src/main.rs`**

```rust
// Detect close stop pairs during initialization
#[derive(Clone, Copy)]
struct CloseStopPair {
    stop_a: u8,
    stop_b: u8,
    distance: DistCm,
}

fn find_close_stops(stops: &[Stop], threshold: DistCm) -> Vec<CloseStopPair> {
    let mut pairs = Vec::new();
    for i in 0..stops.len()-1 {
        let dist = stops[i+1].progress_cm - stops[i].progress_cm;
        if dist < threshold {
            pairs.push(CloseStopPair {
                stop_a: i as u8,
                stop_b: (i+1) as u8,
                distance: dist,
            });
        }
    }
    pairs
}

// During processing, handle close pairs specially
let close_pairs = find_close_stops(&stops, 10000);  // <100m

for &stop_idx in &active_indices {
    // Check if this stop is part of a close pair
    let pair_opt = close_pairs.iter().find(|p| p.stop_a == stop_idx || p.stop_b == stop_idx);

    if let Some(pair) = pair_opt {
        // Process both stops together
        process_close_stop_pair(record, pair, &stops, &mut stop_states);
    } else {
        // Normal single-stop processing
        process_single_stop(record, stop_idx, &stops, &mut stop_states);
    }
}

fn process_close_stop_pair(
    record: &GPSRecord,
    pair: &CloseStopPair,
    stops: &[Stop],
    stop_states: &mut [StopState],
) {
    // Use relaxed probability threshold for close stops
    const THETA_CLOSE_STOP: Prob8 = 170;  // Lower than 191

    // Check both stops
    for &stop_idx in &[pair.stop_a, pair.stop_b] {
        let state = &mut stop_states[stop_idx as usize];
        let stop = &stops[stop_idx as usize];

        // Compute probability
        let prob = arrival_probability(...);

        // Use lower threshold for close stops
        if prob > THETA_CLOSE_STOP {
            let d_to_stop = (record.s_cm - stop.progress_cm).abs();
            if d_to_stop < 5000 {
                // Trigger arrival
                state.fsm_state = FsmState::AtStop;
                // ... trigger arrival event
            }
        }
    }
}
```

---

## 11. COMPARISON OF SOLUTIONS

| Solution | Complexity | Side Effects | Effectiveness | Risk |
|----------|-----------|--------------|---------------|------|
| **1. Multiple Active Stops** | Low | Minimal | ★★★★★ | Low |
| **2. Earlier Departure** | Low | May affect non-close stops | ★★★★☆ | Low |
| **3. Reduce p4 Weight** | Medium | Probability model change | ★★★★☆ | Medium |
| **4. Closeness Bonus** | Low | Affects threshold logic | ★★★★☆ | Low |
| **5. Dwell Initialization** | Medium | Complex state tracking | ★★★☆☆ | Medium |
| **6. Dual-Stop Mode** | High | New code path | ★★★★★ | High |

---

## 12. RECOMMENDED IMPLEMENTATION PLAN

### Phase 1: Quick Win (Solution 1 + Solution 2)

**Goal**: Verify multiple active stops work and implement earlier departure.

**Tasks**:
1. Verify `find_active_stops()` returns multiple stops
2. Check main loop processes all active stops
3. Implement earlier departure (1500cm threshold)
4. Test with tpF805 route

**Expected Outcome**:
- Stop #3 activates earlier (time ~470 instead of 481)
- dwell_time_s has time to accumulate
- Probability reaches threshold

**Acceptance Criteria**:
```
Before: Stop #3 not detected (probability 185 < 191)
After:  Stop #3 detected (probability >= 191)
```

### Phase 2: Probability Fix (Solution 4 if needed)

**Goal**: Add closeness bonus if Phase 1 insufficient.

**Tasks**:
1. Implement closeness bonus (+30)
2. Add next_stop parameter to probability calculation
3. Test with various stop spacings

**Acceptance Criteria**:
```
Stops at 79m apart: Both detected
Stops at 150m apart: Both detected
Stops at 300m apart: Independent behavior
```

### Phase 3: Comprehensive (Solution 6 if needed)

**Goal**: Implement dual-stop processing mode for edge cases.

**Tasks**:
1. Design close stop pair detection
2. Implement pair-based processing
3. Extensive testing across all routes

---

## 13. TESTING STRATEGY

### Unit Tests

```rust
#[test]
fn test_multiple_active_stops() {
    let stops = vec![
        Stop { progress_cm: 100000, corridor_start_cm: 92000, corridor_end_cm: 104000 },
        Stop { progress_cm: 107932, corridor_start_cm: 99932, corridor_end_cm: 111932 },
    ];

    // Position in overlap region
    let active = find_active_stops(103000, &stops);
    assert_eq!(active, vec![0, 1]);  // Both should be active
}

#[test]
fn test_earlier_departure() {
    let mut state = StopState::new(0);
    state.fsm_state = FsmState::AtStop;
    let stop_progress = 10000;

    // At 20m past stop (should depart early if next stop is close)
    state.update(12000, 100, stop_progress, 2000, 200);
    // With next stop at 79m, should transition to Departed
}

#[test]
fn test_closeness_bonus() {
    let stop = Stop { progress_cm: 100000, corridor_start_cm: 92000, corridor_end_cm: 104000 };
    let next_stop = Stop { progress_cm: 107932, /* ... */ };

    let prob = arrival_probability_with_bonus(
        100000,  // At stop
        783,     // High speed
        &stop,
        1,       // Low dwell time
        &gaussian_lut,
        &logistic_lut,
        Some(&next_stop),
    );

    assert!(prob > 191);  // Should trigger with bonus
}
```

### Integration Tests

Create test route with closely-spaced stops:

```json
{
  "stops": [
    {"lat": 25.0, "lon": 121.0, "progress_cm": 100000},
    {"lat": 25.01, "lon": 121.01, "progress_cm": 107932},  // 79m apart
    {"lat": 25.02, "lon": 121.02, "progress_cm": 200000}
  ]
}
```

Test scenarios:
1. Normal speed through both stops
2. Slow speed through both stops
3. Stop at first stop, then proceed
4. Skip first stop, stop at second

---

## 14. REFERENCES

- **Technical Report**: `docs/bus_arrival_tech_report_v8.md`
- **State Machine**: `arrival_detector/src/state_machine.rs`
- **Probability Model**: `arrival_detector/src/probability.rs`
- **Corridor Filter**: `arrival_detector/src/corridor.rs`
- **Test Data**: `test_data/tpF805_*`

---

## 15. APPENDIX: Complete FSM Trace

```
TIME | Stop#State | Dist(cm) | Speed | Prob | p1/p2/p3/p4 | Dwell | JustArrived | Active
-----|------------|----------|-------|------|------------|-------|-------------|-------
 451 | #2Approaching |  -6619 |  733 |   8 |  14/  1/  1/ 25 |  2s | N | Y
 453 | #2Approaching |  -5735 |  753 |  18 |  29/  1/  4/ 51 |  3s | N | Y
 455 | #2Arriving    |  -4715 |  767 |  36 |  59/  0/ 16/ 76 |  4s | N | Y
 457 | #2Arriving    |  -3651 |  782 |  68 | 107/  0/ 49/102 |  5s | N | Y
 459 | #2Arriving    |  -2682 |  759 | 109 | 159/  1/105/127 |  6s | N | Y
 461 | #2Arriving    |  -1804 |  754 | 152 | 207/  1/171/153 |  7s | N | Y
 463 | #2Arriving    |  -1052 |  754 | 182 | 237/  1/223/178 |  8s | N | Y
 465 | #2AtStop      |   -200 |  762 | 201 | 254/  0/253/204 |  9s | Y *** | Y
 467 | #2AtStop      |    757 |  803 | 195 | 246/  0/237/229 |  9s | N | Y
 469 | #2AtStop      |   1692 |  841 | 162 | 211/  0/178/229 |  9s | N | Y
 471 | #2AtStop      |   2583 |  861 | 123 | 164/  0/112/229 |  9s | N | Y
 473 | #2AtStop      |   3514 |  835 |  84 | 114/  0/ 55/229 |  9s | N | Y
 475 | NO ACTIVE STOPS
 477 | NO ACTIVE STOPS
 479 | NO ACTIVE STOPS
 481 | #3Arriving    |   -907 |  756 | 169 | 241/  1/230/  0 |  1s | N | Y
 483 | #3Arriving    |     -7 |  783 | 185 | 255/  0/255/ 25 |  2s | N | Y
 485 | #3Arriving    |    875 |  805 | 175 | 242/  0/231/ 51 |  3s | N | Y
 487 | #3Arriving    |   1587 |  780 | 153 | 217/  0/187/ 76 |  4s | N | Y
 489 | #3Arriving    |   2359 |  797 | 121 | 178/  0/128/102 |  5s | N | Y
 491 | #3Arriving    |   3198 |  757 |  87 | 130/  1/ 71/127 |  6s | N | Y
 493 | #3Arriving    |   3814 |  742 |  67 |  99/  1/ 41/153 |  7s | N | Y
```

---

*Document Version: 1.0*
*Date: 2026-03-24*
*Author: Analysis of tpF805 route, Stop #2 → Stop #3 segment*
