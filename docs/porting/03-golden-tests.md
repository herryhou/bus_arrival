# Golden Tests - End-to-End Validation

## Overview

Golden tests feed the same NMEA input to both the reference implementation (Rust) and the ported implementation, then compare outputs.

## Test Data Catalog

### ty225_normal
**Scenario:** Baseline normal operation
**Duration:** ~30 minutes
**Characteristics:**
- Normal GPS quality (HDOP 1.5-3.0)
- All stops detected correctly
- No GPS jumps or outages

**Files:**
- `ty225_normal_nmea.txt` - NMEA input
- `ty225_normal_gt.json` - Ground truth arrivals
- `ty225_normal_trace_v2.jsonl` - Reference output

**Expected Results:**
- 22 stops detected
- 0 false positives
- 0 false negatives
- 97.8% accuracy

### ty225_jump
**Scenario:** GPS jump recovery
**Duration:** ~5 minutes
**Characteristics:**
- Single GPS jump of ~80m at tick 123
- Speed constraint filter should reject
- Kalman filter maintains smooth estimate

**Files:**
- `ty225_jump_nmea.txt` - NMEA input (with jump)
- `ty225_jump_gt.json` - Ground truth
- `ty225_jump_trace_v2.jsonl` - Reference output

**Expected Results:**
- Jump rejected (no spurious arrival)
- Position error < 10m after 3 ticks
- All arrivals detected correctly

### ty225_drift
**Scenario:** GPS drift handling
**Duration:** ~10 minutes
**Characteristics:**
- Urban canyon conditions
- GPS position drifts ±30m
- Kalman filter reduces noise

**Files:**
- `ty225_drift_nmea.txt` - NMEA input
- `ty225_drift_gt.json` - Ground truth
- `ty225_drift_trace_v2.jsonl` - Reference output

**Expected Results:**
- Smoothed position estimate
- No arrival triggers from drift alone
- Probability model prevents false positives

## Validation Procedure

### Step 1: Run Reference Implementation

```bash
# Using Rust pipeline
cargo run --release -- \
  ROUTE_NAME=ty225 \
  SCENARIO=normal \
  INPUT=test_data/ty225_normal_nmea.txt
```

Output: `trace_v2.jsonl` (reference)

### Step 2: Run Ported Implementation

```bash
# Example: iOS implementation
./ios_pipeline \
  --route test_data/ty225_route.json \
  --input test_data/ty225_normal_nmea.txt \
  --output ported_trace.jsonl
```

### Step 3: Compare Arrivals

Extract arrival events from both traces:

```python
import json

def extract_arrivals(trace_file):
    arrivals = []
    with open(trace_file) as f:
        for line in f:
            tick = json.loads(line)
            for stop in tick['detection_state']['stop_states']:
                if stop['fsm_state'] == 'AtStop' and stop.get('just_arrived'):
                    arrivals.append({
                        'stop_idx': stop['index'],
                        'time': tick['time'],
                        'dwell_s': stop.get('dwell_time_s', 0)
                    })
    return arrivals

ref_arrivals = extract_arrivals('reference_trace.jsonl')
port_arrivals = extract_arrivals('ported_trace.jsonl')
```

Compare against ground truth:

```python
def validate(arrivals, gt_file):
    with open(gt_file) as f:
        gt = json.load(f)

    gt_by_stop = {s['stop_idx']: s['arrivals'] for s in gt['stops']}

    correct = 0
    false_pos = 0
    false_neg = 0

    for stop_idx, stop_arrivals in enumerate(arrivals_by_stop):
        expected = gt_by_stop.get(stop_idx, [])

        # Match arrivals by time (±5 seconds)
        for arr in stop_arrivals:
            matched = False
            for exp in expected:
                if abs(arr['time'] - exp['time']) <= 5:
                    correct += 1
                    matched = True
                    break
            if not matched:
                false_pos += 1

        for exp in expected:
            matched = any(abs(arr['time'] - exp['time']) <= 5
                         for arr in stop_arrivals)
            if not matched:
                false_neg += 1

    total = correct + false_pos + false_neg
    accuracy = correct / total if total > 0 else 0

    return {
        'accuracy': accuracy,
        'correct': correct,
        'false_positives': false_pos,
        'false_negatives': false_neg
    }
```

### Step 4: Debug Trace Diff

When validation fails, compare per-tick state:

```python
def compare_traces(ref_trace, port_trace):
    with open(ref_trace) as rf, open(port_trace) as pf:
        for r_line, p_line in zip(rf, pf):
            r_tick = json.loads(r_line)
            p_tick = json.loads(p_line)

            # Compare Kalman state
            s_diff = abs(r_tick['kalman_state']['s_cm'] -
                        p_tick['kalman_state']['s_cm'])
            if s_diff > 1000:  # 10m threshold
                print(f"Tick {r_tick['time']}: s_cm diff = {s_diff} cm")

            # Compare stop states
            for r_stop, p_stop in zip(
                r_tick['detection_state']['stop_states'],
                p_tick['detection_state']['stop_states']
            ):
                if r_stop['fsm_state'] != p_stop['fsm_state']:
                    print(f"Stop {r_stop['index']}: "
                          f"{r_stop['fsm_state']} → {p_stop['fsm_state']}")
```

## Accuracy Metrics

### Target Performance

| Metric | Target | Rationale |
|--------|--------|-----------|
| Arrival detection | ≥ 97% | Production threshold |
| False positive rate | < 3% | User experience |
| False negative rate | < 3% | Operational requirement |
| Position error (steady) | < 10m | Kalman effectiveness |
| Position error (drift) | < 15m | GPS noise handling |

### Per-Scenario Acceptance

| Scenario | Min Accuracy | Notes |
|----------|-------------|-------|
| ty225_normal | ≥ 97% | Baseline |
| ty225_jump | ≥ 95% | Single anomaly acceptable |
| ty225_drift | ≥ 95% | Noise tolerance |

### Failure Diagnosis

| Symptom | Likely Cause | Check |
|---------|--------------|-------|
| High false positive | Probability threshold too low | Verify P > 191 |
| High false negative | Corridor too narrow | Check corridor boundaries |
| Position drift | Kalman gain too high | Verify K_s values |
| Missed stops | Map matching error | Check heading gate |
| Duplicate arrivals | FSM reactivation | Verify one-time rule |
