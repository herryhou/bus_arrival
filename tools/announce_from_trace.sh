#!/bin/bash
# Extract announce events from grouped trace v2 JSONL
# Usage: ./tools/announce_from_trace.sh <trace_v2.jsonl>

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

jq -c 'select(.corridor.active_stops and (.corridor.active_stops | length > 0)) |
    {time: .gps.time_ms, stop_idx: .corridor.active_stops[0], s_cm: .kalman.s_cm, v_cms: .kalman.v_cms}' \
  "$TRACE_FILE"
