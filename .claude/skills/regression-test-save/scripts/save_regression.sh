#!/bin/bash
# save_regression.sh — Save a failing test case as a permanent regression test
#
# Usage:
#   ./save_regression.sh <case-name> "<description>"
#
# Example:
#   ./save_regression.sh ty225_stop_skip_backtrack "Progress stuck at stop 2 after detour re-entry"
#
# What it does:
#   1. Copies ty225_short_detour_nmea.txt to test_data/regression/
#   2. Copies trace.jsonl to test_data/regression/ (for documentation)
#   3. Prompts for bug details and updates test_data/regression/README.md
#   4. Creates a test template in crates/pipeline/tests/regression_tests.rs

set -euo pipefail

REGRESSION_DIR="test_data/regression"
EXPECTED_DIR="test_data/expected"
PIPELINE_DIR="crates/pipeline/tests"

# Check arguments
if [ $# -lt 1 ]; then
    echo "Usage: $0 <case-name> \"<bug-description>\""
    echo ""
    echo "Example:"
    echo "  $0 ty225_stop_skip_backtrack \"Progress stuck at stop 2 after detour re-entry\""
    exit 1
fi

CASE_NAME="$1"
BUG_DESCRIPTION="${2:-}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Source files (assumes we just ran make run-detour)
NMEA_SOURCE="test_data/ty225_short_detour_nmea.txt"
TRACE_SOURCE="test_data/ty225_short_detour_trace.jsonl"
ROUTE_BIN="test_data/ty225_short_detour.bin"

# Destination files
NMEA_DEST="$REGRESSION_DIR/ty225_short_detour_${CASE_NAME}.txt"
TRACE_DEST="$REGRESSION_DIR/ty225_short_detour_${CASE_NAME}_trace.jsonl"

# Check if source files exist
if [ ! -f "$NMEA_SOURCE" ]; then
    echo "Error: NMEA source file not found: $NMEA_SOURCE"
    echo "Run 'make run-detour' first to generate test data"
    exit 1
fi

# Create directories if needed
mkdir -p "$REGRESSION_DIR"
mkdir -p "$EXPECTED_DIR"

# Copy files
echo "Saving regression test case: $CASE_NAME"
echo ""
cp "$NMEA_SOURCE" "$NMEA_DEST"
echo "✓ Saved NMEA: $NMEA_DEST"

if [ -f "$TRACE_SOURCE" ]; then
    cp "$TRACE_SOURCE" "$TRACE_DEST"
    echo "✓ Saved trace: $TRACE_DEST"
fi

# Prompt for bug details if not provided
if [ -z "$BUG_DESCRIPTION" ]; then
    echo ""
    echo "Enter a brief bug description (one line):"
    read -r BUG_DESCRIPTION
fi

# Get root cause and fix location
echo ""
echo "Root cause (why it happened):"
read -r ROOT_CAUSE

echo ""
echo "Fix (which file/function):"
read -r FIX_LOCATION

# Update README
README_FILE="$REGRESSION_DIR/README.md"
TEMP_README="/tmp/regression_readme_$TIMESTAMP.md"

# Create new entry
cat > "$TEMP_README" << EOF
### $CASE_NAME

**File:** \`ty225_short_detour_${CASE_NAME}.txt\`
**Bug:** $BUG_DESCRIPTION
**Root Cause:** $ROOT_CAUSE
**Fix:** $FIX_LOCATION
**Test:** \`test_${CASE_NAME}()\`
**Added:** $(date +%Y-%m-%d)

---

EOF

# Insert after "Current Regression Cases" header
if grep -q "Current Regression Cases" "$README_FILE"; then
    # Insert after the header section
    awk -v new_entry="$(cat "$TEMP_README")" '
        /^## Current Regression Cases/ { print; print ""; getline; print new_entry; next }
        { print }
    ' "$README_FILE" > "${README_FILE}.tmp" && mv "${README_FILE}.tmp" "$README_FILE"
else
    # First entry - create the section
    cat "$TEMP_README" >> "$README_FILE"
fi

rm "$TEMP_README"
echo "✓ Updated $README_FILE"

# Create test template
TEST_FILE="$PIPELINE_DIR/regression_tests.rs"
if [ ! -f "$TEST_FILE" ]; then
    # Create new test file with module structure
    cat > "$TEST_FILE" << 'EOF'
//! Regression tests for bugs found during testing
//!
//! Each test documents a specific bug that was found and fixed.
//! Tests use saved NMEA files from test_data/regression/.

use std::fs;
use std::io::BufRead;

mod common;

// Helper function to run pipeline with regression test data
fn run_regression_test(nmea_file: &str, route_bin: &str) -> (Vec<String>, Vec<String>) {
    // Load route data
    let route_bytes = fs::read(route_bin)
        .expect("Failed to load route data");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse route data");

    // Load NMEA
    let nmea_file = fs::File::open(nmea_file)
        .expect("Failed to open NMEA file");
    let reader = std::io::BufReader::new(nmea_file);

    // TODO: Process NMEA through pipeline
    // For now, return empty vectors
    (Vec::new(), Vec::new())
}
EOF
fi

# Append test case
cat >> "$TEST_FILE" << EOF

#[test]
fn test_${CASE_NAME}() {
    // Bug: $BUG_DESCRIPTION
    // Root cause: $ROOT_CAUSE
    // Fix: $FIX_LOCATION
    //
    // This test ensures the bug does not regress.

    let nmea_file = "../../../test_data/regression/ty225_short_detour_${CASE_NAME}.txt";
    let route_bin = "../../../test_data/ty225_short_detour.bin";

    // TODO: Implement test assertions
    // - Load and process NMEA
    // - Verify expected behavior
    // - Check for bug symptoms

    let (_arrivals, _trace) = run_regression_test(nmea_file, route_bin);

    // Example assertions (customize for your bug):
    // assert!(progress_after_reentry > 170_000, "Progress should jump to stop 6 area");
    // assert!(stop_idx == 6, "Should detect stop 6 after re-entry");

    // For now, just verify we can load the files
    assert!(fs::metadata(nmea_file).is_ok(), "NMEA file should exist");
    assert!(fs::metadata(route_bin).is_ok(), "Route bin file should exist");
}
EOF

echo "✓ Created test template in $TEST_FILE"
echo ""
echo "Next steps:"
echo "  1. Implement the test assertions in $TEST_FILE"
echo "  2. Fix the bug in the code"
echo "  3. Run: cargo test -p pipeline --test regression_tests -- test_${CASE_NAME}"
echo "  4. Commit the changes"
