#!/bin/bash
# Extract announce events from trace.jsonl
# Usage: ./tools/announce_from_trace.sh <trace.jsonl>

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

jq 'select(.active_stops and (.active_stops | length > 0)) |
    {time, stop_idx: .active_stops[0], s_cm, v_cms}' \
  "$TRACE_FILE"
