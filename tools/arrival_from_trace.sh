#!/bin/bash
# Extract arrival events from grouped trace v2 JSONL
# Usage: ./tools/arrival_from_trace.sh <trace_v2.jsonl>

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <trace_v2.jsonl>" >&2
    exit 1
fi

TRACE_FILE="$1"

if [ ! -f "$TRACE_FILE" ]; then
    echo "Error: File not found: $TRACE_FILE" >&2
    exit 1
fi

jq -c 'select(.stop_states) |
  {time: .gps.time_ms, s_cm: .kalman.s_cm, v_cms: .kalman.v_cms} + .stop_states[] |
  select(.just_arrived == true) |
  {time, stop_idx, s_cm, v_cms, probability}' \
  "$TRACE_FILE"
