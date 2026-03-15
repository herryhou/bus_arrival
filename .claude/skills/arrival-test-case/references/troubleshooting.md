# Troubleshooting Guide

## Critical Bugs and Fixes

### Coordinate System Mismatch (FIXED v0.2.0)

**Status:** ✅ FIXED - Preprocessor now transforms route nodes to grid origin

**Symptoms:**
- Max s_cm < 10,000 cm for multi-km routes
- active_stops array always empty
- 0 arrivals detected despite valid NMEA data
- GPS coordinates project 10+ km outside route

**Detection:**
```bash
# Check if transform was applied
cargo run -p preprocessor -- route.json stops.json output.bin 2>&1 | grep "Transformed"
# Should show: "Transformed route nodes to grid origin (offset: X, Y)"

# Check s_cm range
MAX_S=$(jq -r '.s_cm' output_sim.jsonl | sort -n | tail -1)
if [ $MAX_S -lt 10000 ]; then
  echo "ERROR: Coordinate system mismatch detected!"
  echo "Max s_cm: $MAX_S cm (expected: 200000+ cm for 2km route)"
fi
```

**Solution:**
1. Ensure latest preprocessor: `cargo build -p preprocessor`
2. Regenerate binary file with fixed preprocessor
3. Verify transform message appears in output

### Heading Overflow Bug (FIXED)

**Status:** ✅ FIXED - Simulator converts NMEA 0-360° to i16 range

**Symptoms:**
- Heading shows 32767 (sentinel value) in simulator output
- Visualizer shows random heading arrows

**Detection:**
```bash
# Check for overflow sentinel
grep -c '"heading_cdeg": 32767' output_sim.jsonl
# Should return 0
```

**Solution:** Already fixed in simulator - ensure latest version

## Common Issues

### "Processed 0 GPS Updates"

**Cause:** NMEA file doesn't match route geometry used for binary file

**Detection:**
```bash
# Simulator output shows:
# Processed 0 GPS updates
```

**Solution:**
```bash
# Regenerate NMEA with same route JSON
cd tools/gen_nmea
node gen_nmea.js generate \
  --route ../../test_data/route.json \
  --stops ../../test_data/stops.json \
  --out-nmea ../../test_data/test.nmea \
  --out-gt ../../test_data/test_gt.json
```

### Missing active_stops Field

**Cause:** Old simulator output format or incompatible version

**Detection:**
```bash
# Check if field exists
jq -r 'has("active_stops")' output_sim.jsonl | sort -u
# Should show: true
```

**Solution:** Ensure latest simulator version: `cargo build -p simulator`

### No Events in Visualizer

**Cause:** Using simulator output instead of arrival detector trace

**Detection:** Visualizer shows bus movement but no ARRIVAL/DEPARTURE events

**Solution:**
```bash
# Use --trace output from arrival_detector
cargo run -p arrival_detector -- \
  sim.jsonl route.bin arrivals.jsonl \
  --trace trace.jsonl  # Use trace.jsonl in visualizer
```

### Fewer Arrivals Than Expected

**Cause:** Short dwell times, high speeds, or GPS noise

**Detection:**
```bash
# Compare detected vs expected
DETECTED=$(wc -l < arrivals.jsonl)
EXPECTED=$(jq '. | length' ground_truth.json)
echo "Detected: $DETECTED, Expected: $EXPECTED"
```

**Debug:**
```bash
# Check speed at missed stop
jq -r 'select(.active_stops | index(2)) | {time, s_cm, v_cms}' sim.jsonl

# If v_cms > 100 (1 m/s) throughout stop, bus didn't slow down enough
```

**Possible solutions:**
1. Increase dwell time in ground truth
2. Adjust arrival detector sensitivity
3. Check GPS noise levels (increase sigmaM for testing)

## Verification Checklist

Before considering a test case complete, verify:

- [ ] Preprocessor shows "Transformed route nodes to grid origin"
- [ ] Max s_cm > 80% of expected route length
- [ ] active_stops shows both 0 and 1 (empty + populated)
- [ ] No heading overflow (grep -c 32767 returns 0)
- [ ] At least 50% of stops detected (better for normal scenarios)
- [ ] Visualizer shows bus on route path
- [ ] Visualizer shows ARRIVAL events

## Quick Diagnostics

```bash
#!/bin/bash
# Run this to diagnose common issues

echo "=== Bus Arrival Test Diagnostics ==="

# 1. Check preprocessor version
echo -n "Preprocessor transform: "
cargo run -p preprocessor -- route.json stops.json test.bin 2>&1 | grep -q "Transformed" && echo "✓ PASS" || echo "✗ FAIL - Rebuild preprocessor"

# 2. Check simulator output
echo -n "Simulator GPS updates: "
UPDATES=$(tail -1 test_sim.jsonl | jq -r '.time' 2>/dev/null)
if [ "$UPDATES" -gt 100 ]; then
  echo "✓ PASS ($UPDATES updates)"
else
  echo "✗ FAIL (only $UPDATES updates)"
fi

# 3. Check s_cm range
echo -n "Route progress (s_cm): "
MAX_S=$(jq -r '.s_cm' test_sim.jsonl | sort -n | tail -1)
if [ "$MAX_S" -gt 100000 ]; then
  echo "✓ PASS ($(($MAX_S / 100)) meters)"
else
  echo "✗ FAIL ($(($MAX_S / 100)) meters - expected 2000+)"
fi

# 4. Check active_stops
echo -n "Stop detection (active_stops): "
HAS_BOTH=$(jq -r '.active_stops | length' test_sim.jsonl | sort -u | wc -l)
if [ "$HAS_BOTH" -eq 2 ]; then
  echo "✓ PASS (has 0 and 1)"
else
  echo "✗ FAIL (only has: $(jq -r '.active_stops | length' test_sim.jsonl | sort -u | tr '\n' ' '))"
fi

# 5. Check arrivals
echo -n "Arrival detection: "
ARRIVALS=$(wc -l < test_arrivals.jsonl 2>/dev/null)
if [ "$ARRIVALS" -gt 0 ]; then
  echo "✓ PASS ($ARRIVALS arrivals)"
else
  echo "✗ FAIL (0 arrivals)"
fi

echo "=== End Diagnostics ==="
```

## Getting Help

If issues persist:

1. Check memory bank for previous solutions:
   ```
   /mem-search "coordinate system bug"
   /mem-search "arrival detection failed"
   ```

2. Review test case examples in `.claude/skills/arrival-test-case/examples/`

3. Run the diagnostics script above to identify specific issues

4. Check git history for recent fixes:
   ```
   git log --oneline --all --grep="coordinate\|arrival\|preprocessor"
   ```
