# CLAUDE.md

## Working Style

### 1. Think Before Coding
Don't assume. Don't hide confusion. Surface tradeoffs.
- State assumptions explicitly. If uncertain, ask.
- Present multiple interpretations — don't pick silently.
- Push back when simpler approaches exist.
- Stop when unclear. Name what's confusing. Ask.

### 2. Simplicity First
Minimum code that solves the problem. Nothing speculative.
- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" that wasn't requested.
- No error handling for impossible scenarios.
- If 200 lines could be 50, rewrite it.

### 3. Surgical Changes
Touch only what you must. Clean up only your own mess.
- Don't "improve" adjacent code or formatting.
- Don't refactor what isn't broken.
- Match existing style, even if you'd do it differently.
- Remove orphans your changes created; leave pre-existing dead code alone.

### 4. Goal-Driven Execution
Define success criteria. Loop until verified.
- "Add validation" → Write tests for invalid inputs, then make them pass
- "Fix the bug" → Write a test that reproduces it, then make it pass
- "Refactor X" → Ensure tests pass before and after

---

## Source of Truth

**MUST read before any work:**
- `bus_arrival_tech_report_v8.md` — Detailed design doc with rationale, algorithms, and data structures
- `docs/SPEC.md` — Master spec index

## Build Commands

```bash
# Build all
make build

# Run pipeline (NMEA + route_data → trace.jsonl)
make run ROUTE_NAME=ty225 SCENARIO=normal

# Generate route data from GeoJSON
cargo run -p preprocessor -- route.json stops.json output.bin

# Run pipeline directly
cargo run -p pipeline -- nmea.txt route.bin

# Extract from trace
./tools/arrival_from_trace.sh trace.jsonl > arrivals.jsonl
./tools/announce_from_trace.sh trace.jsonl > announce.jsonl

# Tests
cargo test
cargo test -p pipeline
```

## Architecture

3-phase pipeline: NMEA → GPS localization (Kalman + map matching) → Bayesian arrival detection → `trace.jsonl`

```
crates/
├── shared/           # Types, binary format
├── preprocessor/     # Route simplification
├── pipeline/         # GPS + detection
├── trace_validator/  # Validation tool
└── pico2-firmware/   # Embedded (RP2350, no_std)
```

## Architecture

The system processes GPS NMEA data to detect bus arrivals using a 3-phase pipeline:

**Phase 1 (preprocessor):** Route preprocessing and simplification (Douglas-Peucker algorithm)
- Input: GeoJSON route + stops
- Output: Binary route data (`route_data.bin`) with precomputed coefficients

**Phase 2 (gps_processor):** GPS localization with Kalman filtering and map matching
- Spatial grid index, heading-constrained map matching, 1D Kalman filter
- Dead-reckoning for GPS outages

**Phase 3 (detection):** Bayesian arrival detection with finite state machine
- 4-feature probability model (distance, speed, progress error, dwell time)
- Stop corridor filtering, state machine (Approaching → Arriving → AtStop → Departed)
- Stop index recovery after GPS anomalies

**Output:** `trace.jsonl` (complete state machine trace with arrivals, departures, and all intermediate states)

## Workspace Structure

```
crates/
├── shared/           # Shared types and binary format (RouteNode, Stop, binfile)
├── preprocessor/     # Phase 1: Route simplification and binary packing
├── pipeline/         # Phase 2 + 3: Unified pipeline (gps_processor + detection)
│   ├── gps_processor/  # GPS localization library
│   └── detection/      # Arrival detection library
├── trace_validator/  # Trace validation tool (compare vs ground truth)
└── pico2-firmware/   # Embedded firmware (RP2350, no_std, embassy-rp)
```

## Firmware (pico2-firmware)

**3-layer architecture with clear component boundaries:**

### Component Layers
- **Parser Layer:** `NmeaParser` component (NMEA → GpsPoint)
- **Estimation Layer:** `EstimationState` with isolated Kalman/DR pipeline
- **Control Layer:** `SystemState` orchestrating `ModeMachine`, estimation, detection, recovery

### Key Components
- **NmeaParser:** `feed_sentence()` → `Option<GpsPoint>` (pure state update)
- **ModeMachine:** `update(ModeInput)` → `ModeOutput` (pure state machine)
- **Estimation:** `estimate(EstimationInput)` → `EstimationOutput` (isolated pipeline)
- **Recovery:** `recover(RecoveryInput)` → `Option<usize>` (pure function)

### Principles
- **Isolation:** Estimation has no access to mode/stop state
- **Single Transition:** ModeMachine enforces one transition per tick
- **Explicit Boundaries:** Each component has well-defined inputs/outputs
- **Testability:** Components can be tested in isolation

### Mode System
- **Normal:** Kalman-filtered position, arrival detection enabled
- **OffRoute:** Position frozen, detection suppressed
- **Recovering:** Raw GPS position, recovery search active

### Entry Point
- `main.rs` uses `SystemState::tick(gps, est_state)` → `Option<ArrivalEvent>`
- Old `state::State` deprecated but still available

## Key Constraints

- **Integer-only** (no FPU on RP2350)
- **Semantic types:** `DistCm` (i32), `SpeedCms` (i32), `HeadCdeg` (i16), `Prob8` (u8)
- **XIP:** Route data in Flash, zero-copy
- **Budget:** < 8% CPU @ 150MHz (1Hz GPS), ~34 KB Flash, < 1 KB SRAM

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (60-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk go test             # Go test failures only (90%)
rtk jest                # Jest failures only (99.5%)
rtk vitest              # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk pytest              # Python test failures only (90%)
rtk rake test           # Ruby test failures only (90%)
rtk rspec               # RSpec test failures only (60%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%). Format flags (-c, -l, -L, -o, -Z) run raw.
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
