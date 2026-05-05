#!/bin/bash
# Extract arrival events from trace.jsonl
# Usage: ./tools/arrival_from_trace.sh <trace.jsonl>

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <trace.jsonl>" >&2
    exit 1
fi

TRACE_FILE="$1"

if [ ! -f "$TRACE_FILE" ]; then
    echo "Error: File not found: $TRACE_FILE" >&2
    exit 1
fi

jq 'select(.stop_states) |
  .stop_states[] |
  select(.just_arrived == true) |
  {time: .time, stop_idx, s_cm, v_cms, probability}' \
  "$TRACE_FILE"
