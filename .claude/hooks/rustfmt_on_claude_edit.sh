#!/bin/bash
set -euo pipefail

# Read JSON from stdin
if ! json_input=$(cat); then
  echo "⚠️ Failed to read JSON input, skipping."
  exit 0
fi

# Ensure jq is installed
if ! command -v jq >/dev/null; then
  echo "⚠️ jq not found, skipping rustfmt."
  exit 0
fi

# Extract file_path
file=$(echo "$json_input" | jq -r '.tool_input.file_path // empty')

# If empty or not a Rust file, skip
if [[ -z "$file" ]] || [[ ! "$file" == *.rs ]]; then
  exit 0
fi

# Make absolute if relative
if [[ "$file" != /* ]]; then
  file="$(pwd)/$file"
fi

# Only format if file exists
if [[ -f "$file" ]]; then
  echo "🛠️ Formatting with rustfmt: $file"
  rustfmt "$file"
else
  echo "⚠️ File not found (skipping rustfmt): $file"
fi
