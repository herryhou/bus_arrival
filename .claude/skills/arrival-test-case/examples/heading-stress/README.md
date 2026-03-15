# Heading Stress Test Example

This test case creates route segments that generate headings around the 180° boundary, testing the overflow fix.

## Route Design

The route creates segments that produce headings crossing from 179° to 181° (which wraps to -17900 cdeg).

## Usage

```bash
# 1. Generate NMEA
cd tools/gen_nmea
node gen_nmea.js generate \
  --route ~/.claude/skills/arrival-test-case/examples/heading-stress/route.json \
  --stops ~/.claude/skills/arrival-test-case/examples/heading-stress/stops.json \
  --out-nmea heading_stress.nmea

# 2. Run simulator (requires existing route.bin)
cargo run -p simulator -- heading_stress.nmea route.bin heading_stress_sim.jsonl

# 3. Verify no overflow
grep -c '32767' heading_stress_sim.jsonl  # should be 0

# 4. Check heading range
jq -r '.heading_cdeg' heading_stress_sim.jsonl | sort -n | grep -v null
```

## Expected Results

- All `heading_cdeg` values between -18000 and 18000
- No 32767 overflow values
- Simulator processes >0 GPS updates
