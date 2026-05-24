# CLAUDE.md - Bus Arrival Detection

Multi-part project: Rust pipeline + firmware, Android app, Web visualizer.

## Project Parts

- **Rust core** (this directory): Preprocessing, GPS pipeline, embedded firmware
- **Android app** (`android/`): Tablet display app — see `android/CLAUDE.md`
- **Visualizer** (`visualizer/`): Web trace viewer — see `visualizer/CLAUDE.md`

## Source of Truth

**Read first:**
- `docs/bus_arrival_tech_report_v8.md` — Design, algorithms, data structures
- `docs/SPEC.md` — Master spec index

## Quick Start (Rust)

```bash
make build                           # Build all
make run ROUTE_NAME=ty225 SCENARIO=normal  # Run pipeline
cargo test                           # Run tests
```

## Architecture

3-phase pipeline: NMEA → GPS (Kalman + map matching) → Bayesian detection → `trace_v2.jsonl`

```
crates/
├── shared/           # Types, binary format
├── preprocessor/     # Route simplification
├── pipeline/         # GPS + detection
├── trace_validator/  # Validation tool
└── pico2-firmware/   # Embedded (RP2350, no_std)
```

## Key Constraints

- **Integer-only** (no FPU on RP2350)
- **Semantic types:** `DistCm`, `SpeedCms`, `HeadCdeg`, `Prob8`
- **Budget:** < 8% CPU @ 150MHz, ~34 KB Flash, < 1 KB SRAM

## Working Style

1. Think before coding — state assumptions, surface tradeoffs
2. Simplicity first — minimum code, no speculative features
3. Surgical changes — touch only what you must
4. Goal-driven — define success, loop until verified

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer)

**Golden Rule:** Always prefix commands with `rtk`. Even in chains: `rtk git add . && rtk git commit -m "msg"`

Key commands:
- `rtk cargo build/test` — Build/test (80-90% savings)
- `rtk git status/log/diff` — Git ops (59-80% savings)
- `rtk gain` — View savings stats
- `rtk discover` — Find missed opportunities

Full reference: `~/.claude/RTK.md` or `rtk gain --history`
<!-- /rtk-instructions -->

<!-- code-review-graph MCP -->
## MCP: code-review-graph

**Use graph tools FIRST** (faster, cheaper, structural context):
- `semantic_search_nodes` — Find functions/classes
- `detect_changes` — Review code changes
- `get_impact_radius` — Understand blast radius
- `query_graph` — Trace callers, callees, tests

Fall back to rg/Grep/Glob/Read only when graph doesn't cover it.
<!-- /code-review-graph -->
