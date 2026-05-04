# Expected Outputs

This directory contains expected outputs for regression tests.

## Purpose

Each regression test may have:
- `<case-name>_arrivals.jsonl` — Expected arrival/departure events
- `<case-name>_trace.jsonl` — Expected debug trace state
- `<case-name>_gt.json` — Ground truth (from gen_nmea)

## When to Use

**For deterministic bugs:** Use expected outputs when the bug produces clearly wrong output that should be fixed to a known correct state.

**For state validation:** For tests that check internal state (progress, stop_idx, mode), you don't need expected outputs — the test assertions are sufficient.

## Generating Expected Outputs

After fixing a bug:

```bash
# Run the pipeline with the regression NMEA
cargo run --release -p pipeline -- \
    test_data/regression/ty225_short_detour_<case>.txt \
    test_data/ty225_short_detour.bin \
    test_data/regression/temp_output.jsonl \
    --trace test_data/regression/temp_trace.jsonl

# Verify the output is correct, then save it as expected
mv test_data/regression/temp_output.jsonl test_data/expected/<case>_arrivals.jsonl
mv test_data/regression/temp_trace.jsonl test_data/expected/<case>_trace.jsonl
```

## Omitting Expected Outputs

If a regression test only validates:
- Internal state (progress, stop_idx)
- No crashes or hangs
- Mode transitions

...then you don't need expected outputs. The test assertions are sufficient.
