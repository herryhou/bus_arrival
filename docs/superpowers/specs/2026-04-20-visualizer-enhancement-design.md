# Visualizer Enhancement Design

**Date:** 2026-04-20
**Status:** Approved
**Related:** Visualizer probability visualization + smart file loading

## Overview

Enhance the bus arrival visualizer with two key improvements:
1. Display probability status for all stops on the linear route view
2. Auto-load matching trace files when a route `.bin` file is selected

## Goals

- **Visual clarity:** See arrival probabilities and FSM states for all stops at a glance
- **Better UX:** Reduce friction when loading route + trace data pairs
- **Detailed drill-down:** Hover/click for full probability breakdown

## Section 1: File Loading Enhancement

### Goal
When user selects a `.bin` file, automatically scan the same directory for a matching `*_trace.jsonl` file.

### Implementation

**Use File System Access API:**
- Replace `<input type="file">` with `window.showOpenFilePicker()`
- Returns file handles with path information for directory scanning

**Filename matching logic:**
```
Selected: ty225_short_detour.bin
Looking for: ty225_short_detour_trace.jsonl
Pattern: {basename}_trace.jsonl

Supported variants:
- {basename}_trace.jsonl (primary)
- {basename}.jsonl (fallback)
```

**UI feedback:**
- Success: "Found matching trace: {filename}" with auto-load checkbox (default checked)
- No match: "No matching trace file found in directory" with manual upload option
- Override: User can uncheck auto-selection and pick manually

**Fallback behavior:**
- If File System Access API not supported (Safari, older browsers)
- Fall back to traditional dual file inputs with drag-drop enhancement

---

## Section 2: LinearRouteWidget Probability Visualization

### Goal
Show probability + FSM state for all stops on the linear route, with detailed breakdown on hover/click.

### Visual Design

```
Route View (horizontal, scrollable)
┌────────────────────────────────────────────────────────────┐
│ ○────○────○────●────○────○────●────○                      │
│ 1    2    3    4    5    6    7    8                       │
│      127  191  255  191  127                               │
│           App  Arr  AtS                                    │
└────────────────────────────────────────────────────────────┘

Legend:
○ = Stop circle (12px, color by probability)
● = Selected stop (larger ring + white fill)
127, 191, 255 = Probability values displayed below stops
App/Arr/AtS = FSM state badges (abbreviated)
```

### Color Scale (Probability)

| Probability Range | Color | Label |
|-------------------|-------|-------|
| 0-63 | Gray (#666) | None |
| 64-127 | Yellow (#eab308) | Low |
| 128-190 | Orange (#f97316) | Medium |
| 191-255 | Green (#22c55e) | High/Arrived |

### FSM State Badges (Abbreviated)

| State | Badge | Full Name |
|-------|-------|-----------|
| Idle | `Idl` | Idle |
| Approaching | `App` | Approaching |
| Arriving | `Arr` | Arriving |
| AtStop | `AtS` | AtStop |
| Departed | `Dep` | Departed |
| TripComplete | `Com` | TripComplete |

### Component Structure

```
LinearRouteWidget.svelte (enhanced)
├── StopMarker.svelte (NEW - individual stop rendering)
└── StopTooltip.svelte (NEW - detailed breakdown popup)
```

**StopMarker.svelte:**
- Renders individual stop circle with probability color
- Shows probability value below
- Shows FSM state badge below
- Handles hover/click events

**StopTooltip.svelte:**
- Shows detailed probability breakdown (similar to ProbabilityScope)
- Displays: Final P, individual feature scores (p1-p4), distance, speed, dwell
- FSM state transition info
- Positioned near hovered stop marker

---

## Section 3: Data Flow & Architecture

### Component Structure

```
+page.svelte
├── UploadScreen.svelte (NEW - extracted from inline)
│   └── FilePicker.ts (smart file discovery)
├── LinearRouteWidget.svelte (ENHANCED)
│   ├── StopMarker.svelte (NEW)
│   └── StopTooltip.svelte (NEW)
└── [existing components unchanged]
```

### Data Flow

**File loading:**
```
User picks .bin → FilePicker scans directory
→ Finds matching .jsonl → Load both → Parse
→ Set routeData + traceData → Hide upload screen
```

**Probability updates:**
```
Timeline scrub → currentTime changes → currentRecord updates
→ LinearRouteWidget receives new activeStopStates
→ StopMarkers re-render with new probabilities/colors
```

**Tooltip interaction:**
```
Hover stop marker → Get stopState for that stop at currentTime
→ Show StopTooltip with full breakdown → Click to select
```

### State Management

All existing `$state` and `$derived` in `+page.svelte` remain unchanged:
- `routeData`, `traceData`, `currentTime`, `selectedStop`
- New: `hoveredStop` for tooltip state

---

## Section 4: Error Handling

### File Loading Errors

| Error | Handling |
|-------|----------|
| No matching trace found | Show message, offer manual upload |
| Parse error (invalid JSON) | Show specific error with line number |
| File API not supported | Fall back to traditional dual inputs |
| Corrupt binary file | Show error message, suggest regenerating |

### Rendering Errors

| Error | Handling |
|-------|----------|
| Missing probability data | Show gray marker, tooltip says "No data" |
| Invalid FSM state | Show badge "UNK" (unknown) |
| Stop outside time range | Show gray marker, no data available |

---

## Section 5: Implementation Order

1. **Extract components** — Create `StopMarker.svelte` and `StopTooltip.svelte`
2. **Enhance LinearRouteWidget** — Add probability visualization and state badges
3. **Implement FilePicker** — Add directory scanning logic
4. **Add fallback** — Support browsers without File System Access API
5. **Test** — Verify with existing trace files (`ty225_trace.jsonl`, etc.)

---

## Out of Scope

- Map view changes (no modifications to MapView)
- Timeline changes (no modifications to Timeline)
- ProbabilityScope changes (keep as-is for selected stop detail)
- Backend/rust changes (visualizer-only work)

---

## Success Criteria

1. User can select `.bin` file and matching `.jsonl` auto-loads
2. Linear route shows all stops with probability color-coding
3. Hovering any stop shows detailed probability breakdown
4. FSM state transitions are visible on the linear route
5. Fallback works for browsers without File System Access API
