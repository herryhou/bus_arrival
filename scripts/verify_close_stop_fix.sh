#!/bin/bash
set -e

echo "=== Close Stop Fix Verification ==="
echo

# 1. Preprocess route with new corridor logic
echo "1. Preprocessing tpF805 route..."
cargo run -p preprocessor -- \
    test_data/tpF805_route.json \
    test_data/tpF805_stops.json \
    /tmp/tpF805_verify.bin

# 2. Run arrival detector with trace
echo "2. Running arrival detection..."
cargo run -p arrival_detector -- \
    test_data/tpF805_normal_sim.json \
    /tmp/tpF805_verify.bin \
    /tmp/tpF805_verify_output.json \
    --trace /tmp/tpF805_verify_trace.jsonl

# 3. Check which stops were detected
echo "3. Checking detected stops..."
echo "Detected stops:"
grep -o '"stop_idx":[0-9]*' /tmp/tpF805_verify_output.json | sort | uniq -c

# 4. Check Stop #3 detection specifically
echo
echo "4. Verifying Stop #3 detection..."
STOP3_COUNT=$(grep -c '"stop_idx":3' /tmp/tpF805_verify_output.json)

if [ "$STOP3_COUNT" -gt 0 ]; then
    echo "✓ Stop #3 detected ($STOP3_COUNT arrivals)"
else
    echo "✗ FAIL: Stop #3 not detected"

    # Debug: Check if Stop #3 was active in trace
    echo
    echo "Debug: Checking if Stop #3 was ever active..."
    STOP3_ACTIVE=$(grep -c '"active_stops":\[3\]' /tmp/tpF805_verify_trace.jsonl)
    echo "Stop #3 was active in $STOP3_ACTIVE trace records"

    # Check Stop #3 AtStop transitions
    echo
    echo "Debug: Checking Stop #3 AtStop states..."
    grep -A 2 -B 2 '"stop_idx":3' /tmp/tpF805_verify_trace.jsonl | grep -A 2 -B 2 'AtStop' | head -20
    exit 1
fi

# 5. Verify corridor boundaries
echo
echo "5. Checking corridor boundaries..."
echo "Stop #3 corridor boundaries in new route data:"
echo "Note: Binary format - visual verification in trace output"

echo
echo "=== All checks passed ==="