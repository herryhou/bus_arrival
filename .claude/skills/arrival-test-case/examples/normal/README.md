# Normal Test Case Example

A working normal test case for the bus arrival detection system.

## Description

This test case demonstrates a standard bus route with:
- 21 waypoints covering ~3 km
- 4 stops evenly distributed along the route
- Normal GPS conditions (8 sats, HDOP 3.5)
- Expected: 3-4 arrivals detected

## Files

- `route.json` - Route geometry centered at 25.0°N, 121.0°E
- `stops.json` - 4 stops at indices [4, 9, 14, 19]
- `test.nmea` - Generated NMEA GPS data
- `ground_truth.json` - Expected arrival times

## Usage

```bash
# From project root:
cargo run -p preprocessor -- .claude/skills/arrival-test-case/examples/normal/route.json \
  .claude/skills/arrival-test-case/examples/normal/stops.json \
  test_data/normal_example.bin

cargo run -p simulator -- .claude/skills/arrival-test-case/examples/normal/test.nmea \
  test_data/normal_example.bin test_data/normal_example_sim.jsonl

cargo run -p arrival_detector -- test_data/normal_example_sim.jsonl \
  test_data/normal_example.bin test_data/normal_example_arrivals.jsonl \
  --trace test_data/normal_example_trace.jsonl
```

## Expected Results

- Simulator max s_cm: ~250,000-300,000 cm (2.5-3 km)
- active_stops: Shows 0 and 1 (both empty and populated)
- Arrivals detected: 3-4 out of 4 stops
- Some stops may be missed due to short dwell times (< 5s)

## Verification

```bash
# Check route length
MAX_S=$(jq -r '.s_cm' test_data/normal_example_sim.jsonl | sort -n | tail -1)
echo "Route length: $(($MAX_S / 100)) meters"

# Check active stops
jq -r '.active_stops | length' test_data/normal_example_sim.jsonl | sort -u

# Check arrivals
wc -l test_data/normal_example_arrivals.jsonl
```
