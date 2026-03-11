# gen_nmea — GPS Bus Route NMEA Simulator

Agent-friendly CLI for generating NMEA test data and ground truth for GPS bus arrival detection systems.

## Quick Start

```bash
# Generate test data with default settings
./gen_nmea.js generate

# Use a different scenario
./gen_nmea.js generate --json-payload '{"scenario":"drift"}' --json

# Dry run to see what would be generated
./gen_nmea.js generate --scenario jump --dry-run --json
```

## Agent Usage

AI agents should follow this workflow:

1. **Discover schema** → Learn parameter shapes without hardcoding
2. **Build payload** → Construct valid JSON from schema
3. **Execute** → Run with structured output
4. **Parse result** → Check exit code + parse JSON

```bash
# Step 1: Discover the generate command schema
./gen_nmea.js schema generate
# Returns: {"$schema":"http://json-schema.org/...","properties":{...}}

# Step 2: Execute with discovered schema
./gen_nmea.js generate --json-payload '{"route":"route.json","scenario":"normal"}' --json

# Step 3: Parse result (exit 0 = success)
# Output: {"status":"ok","command":"generate","data":{...},"duration_ms":123}
```

## Commands

| Command | Description |
|---------|-------------|
| `generate` | Generate NMEA sentences and ground truth (default) |
| `schema` | Display JSON Schema for parameter discovery |
| `help` | Show usage documentation |

## Generate Options

Via `--json-payload` (recommended for agents):

```json
{
  "route": "route.json",      // Path to route JSON file
  "scenario": "normal",       // normal, drift, jump, outage
  "out_nmea": "test.nmea",    // NMEA output path
  "out_gt": "ground_truth.json" // Ground truth output path
}
```

Or via legacy flags:
- `--route <path>` - Route JSON file
- `--scenario <name>` - Scenario preset
- `--out-nmea <path>` - NMEA output file
- `--out-gt <path>` - Ground truth output file

## Global Flags

| Flag | Purpose |
|------|---------|
| `--json` | Output structured JSON instead of human text |
| `--json-payload '{"key":"value"}'` | Structured input for generate command |
| `--non-interactive` | Fail instead of prompting |
| `--dry-run` | Simulate without writing files |
| `--verbose` | Include debug logs in JSON output |
| `--force` | Skip confirmation prompts |

## Scenarios

| Scenario | HDOP | σ (m) | Sats | Anomalies |
|----------|------|-------|------|-----------|
| `normal` | 3.5 | 18 | 8 | None |
| `drift` | 7.0 | 35 | 5 | Urban canyon drift |
| `jump` | 3.5 | 18 | 8 | 100m+ position jump at 30% |
| `outage` | 3.5 | 18 | 8 | GPS outage on segments 15-17 |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Validation error (invalid parameters) |
| 2 | Execution error (file not found, write failed) |
| 3 | Reserved (future auth errors) |
| 4 | Cancelled by user |

## Output Format

### Success (exit 0)
```json
{
  "status": "ok",
  "command": "generate",
  "data": {
    "scenario": "normal",
    "route_points": 42,
    "stops": 5,
    "nmea_lines": 1234,
    "ground_truth_entries": 5,
    "output_nmea": "test.nmea",
    "output_gt": "ground_truth.json",
    "dry_run": false
  },
  "logs": ["Generated 1234 NMEA lines..."],
  "duration_ms": 123
}
```

### Error (exit 1-2)
```json
{
  "status": "error",
  "code": 2,
  "command": "generate",
  "message": "Cannot read route file: No such file",
  "validation_errors": []
}
```

## Route JSON Format

```json
{
  "route_points": [
    [25.04123, 121.52345],
    [25.04130, 121.52350]
  ],
  "stops": [5, 15, 25, 35, 40],
  "traffic_lights": [10, 20, 30]
}
```

- `route_points`: Array of [lat, lon] pairs defining the bus route
- `stops`: Array of indices into route_points where bus stops occur
- `traffic_lights`: (Optional) Indices where traffic lights cause delays

## Ground Truth Format

The generated `ground_truth.json` contains:

```json
[
  {
    "stop_idx": 0,
    "seg_idx": 5,
    "timestamp": 1700000005,
    "dwell_s": 8
  }
]
```

- `stop_idx`: Sequence number of this stop
- `seg_idx`: Route segment index where stop occurred
- `timestamp`: Unix timestamp of arrival
- `dwell_s`: Seconds bus remained stopped

## Examples

```bash
# Basic generation (human output)
./gen_nmea.js generate --route route.json --scenario drift

# Agent-friendly with full JSON
./gen_nmea.js generate --json-payload '{"route":"data/TY_225.json","scenario":"jump"}' --json

# Dry run with verbose logs
./gen_nmea.js generate --scenario outage --dry-run --verbose --json

# List available commands
./gen_nmea.js schema
# Returns: {"commands":["schema","generate","help"],...}
```
