#!/bin/bash
# Reads JSON from stdin (tool input) and extracts file_path.
# Formats only *.rs files with rustfmt.

set -euo pipefail

# Read JSON from stdin
json_input=$(cat)

# Extract file_path from tool_input
file=$(echo "$json_input" | jq -r '.tool_input.file_path // empty')

# If file is empty or not a Rust file, skip
if [[ -z "$file" ]] || [[ ! "$file" == *.rs ]]; then
  exit 0
fi

# Make absolute if it’s relative
if [[ "$file" != /* ]]; then
  file="$(pwd)/$file"
fi

# If the file exists, run rustfmt
if [[ -f "$file" ]]; then
  echo "🛠️ Formatting with rustfmt: $file"
  rustfmt "$file"
else
  echo "⚠️ File not found (skipping rustfmt): $file"
fi
