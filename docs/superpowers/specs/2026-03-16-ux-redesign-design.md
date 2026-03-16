# Visualizer UI/UX Redesign Design

**Date:** 2026-03-16
**Status:** Approved
**Author:** Claude (with user input)
**Version:** 1.0

## Overview

Redesign the bus arrival visualizer UI to prioritize spatial awareness (map) while making the event feed and stop monitoring more compact and information-dense. Remove non-critical charts (speed) and enhance the timeline with keyboard shortcuts.

## Goals

1. **Larger map view** — Primary spatial context should dominate the layout
2. **Compact sidebar** — Events and stops visible but space-efficient
3. **Cleaner timeline** — Remove speed chart, add keyboard navigation
4. **Maintained functionality** — All existing features preserved, just more compact

## Layout Changes

### New Grid Structure

**Before:**
```css
grid-template-columns: 1.5fr 1.5fr 1fr;  /* Three equal-ish columns */
```

**After:**
```css
grid-template-columns: 2.5fr 1.5fr 1fr;  /* Map bigger, sidebar smaller */
```

**Panel breakdown:**
- **Spatial Panel** (2.5fr): MapView — significantly larger
- **Lab Panel** (1.5fr): ProbabilityScope + FsmInspector — unchanged size
- **Sidebar Panel** (1fr): NEW CompactSidebar — merged Events + Stops
- **Linear Route** (full width): LinearRouteWidget — unchanged
- **Footer** (180px, was 250px): Timeline — speed chart removed

### Visual Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Header (50px)                                              │
├──────────────┬─────────────────────────────────┬────────────┤
│              │                                 │            │
│   Map        │         Lab Panel               │  Compact   │
│   (2.5fr)    │         (1.5fr)                 │  Sidebar   │
│              │                                 │  (1fr)     │
│   Bigger     │   • ProbabilityScope            │            │
│   map view   │   • FsmInspector                │  ┌────────┐ │
│              │   • (empty state msg)           │  │Events  │ │
│              │                                 │  │  50%   │ │
│              │                                 │  ├────────┤ │
│              │                                 │  │Stops   │ │
│              │                                 │  │  50%   │ │
│              │                                 │  └────────┘ │
├──────────────┴─────────────────────────────────┴────────────┤
│  Linear Route Widget (80px)                                │
├─────────────────────────────────────────────────────────────┤
│  Timeline + Playback (180px, was 250px)                   │
│  [▶/⏸] [1x] 00:00:00 / 00:15:30 ━━━━━●━━━━━━━  [␣ ←→ ?] │
└─────────────────────────────────────────────────────────────┘
```

## New Components

### CompactSidebar.svelte

Merges EventLog and AllStopsInspector into a single space-efficient component.

**Props:**
```typescript
interface Props {
  traceData: TraceData;
  currentTime: number;
  v_cms: number;
  selectedStop: number | null;
  onSeek: (time: number) => void;
  onStopSelect: (idx: number) => void;
  onEventClick?: (info: { time: number; stopIdx?: number; state?: FsmState }) => void;
}
```

**Structure:**
```svelte
<div class="compact-sidebar">
  <!-- Top half: Events (50%) -->
  <div class="sidebar-section events-section">
    <div class="section-header">
      <h4>Event Narrative</h4>
      <span class="event-count">{count} events</span>
    </div>
    <div class="section-content">
      <!-- Compact event rows -->
    </div>
  </div>

  <div class="section-divider"></div>

  <!-- Bottom half: All Stops (50%) -->
  <div class="sidebar-section stops-section">
    <div class="section-header">
      <h4>All Stops Monitor</h4>
      <span class="stops-count">{count} active</span>
    </div>
    <div class="section-content">
      <!-- Compact stop cards -->
    </div>
  </div>
</div>
```

**CSS key points:**
- Flex column layout with each section `flex: 1` for 50/50 split
- `min-height: 0` to allow proper shrinking
- Section headers sticky with count badges
- Shared scroll within each section

### Timeline.svelte

Replaces TimelineCharts.svelte with a simpler, keyboard-friendly timeline.

**Props:**
```typescript
interface Props {
  traceData: TraceData;
  selectedStop?: number | null;
  currentTime: number;
  onTimeChange?: (time: number) => void;
}
```

**Features:**
- Play/pause button
- Speed selector (1x/2x/5x/10x)
- Time display (current / total)
- Seek slider
- **Keyboard shortcuts hint**: "␣ play ←→ seek ? help"
- **Keyboard handlers**:
  - Space: Toggle play/pause
  - Arrow Left: Seek -5 seconds
  - Arrow Right: Seek +5 seconds
  - ?: Show help modal (optional)

**No Chart.js dependency** — Uses native HTML/CSS only.

## Component Specifications

### Event Rows (Compact)

**Format:** Inline with alternating row colors

**Elements:**
- Time (HH:MM:SS format, monospace)
- Type badge (ARR/TRN/DEP, 3-char abbreviation)
- Stop number (if applicable)
- State badge (Approaching/Arriving/AtStop/Departed)

**Styling:**
```css
.event-row {
  padding: 0.25rem 0.4rem;     /* Reduced */
  font-size: 0.65rem;          /* Reduced */
  gap: 0.4rem;
}

.event-row.odd { background-color: #0d0d0d; }
.event-row.even { background-color: #111; }
.event-row.current { background-color: rgba(59, 130, 246, 0.2); }
.event-row.highlighted { background-color: rgba(59, 130, 246, 0.3); }
```

**Interaction:**
- Click to seek to that time
- Click emits `onEventClick` with stopIdx and state for map highlighting

### Stop Cards (Compact)

**Format:** All details retained, tighter spacing

**Elements:**
- Header: Stop # + State badge
- Probability gauge (smaller, 14px height)
- Metrics row: distance + dwell time (inline)
- Features: p₁-p₄ values (inline, single line)
- Just arrived badge (if applicable)

**Styling:**
```css
.stop-card-compact {
  padding: 0.4rem 0.5rem;      /* Reduced */
  font-size: 0.7rem;           /* Reduced */
  border-left-width: 3px;      /* Reduced */
  gap: 0.4rem;                 /* Reduced */
}

.prob-gauge-compact {
  height: 14px;                /* Reduced from 20px */
}

.features-compact {
  font-size: 0.6rem;
  display: flex;
  justify-content: space-between;
}
```

**Interaction:**
- Click to select stop (updates `selectedStop` state)
- Active stop highlighted with distinct background
- Selected stop triggers detailed view in Lab Panel

### Lab Panel Behavior

**When no stop selected:**
- Show empty state: "Select a stop from the sidebar to see detailed analysis"

**When stop selected:**
- Show ProbabilityScope (probability gauge visualization)
- Show FsmInspector (state transition history)

## Page Integration (+page.svelte)

**Import changes:**
```typescript
// Remove
import EventLog from '$lib/components/EventLog.svelte';
import AllStopsInspector from '$lib/components/AllStopsInspector.svelte';
import TimelineCharts from '$lib/components/TimelineCharts.svelte';

// Add
import CompactSidebar from '$lib/components/CompactSidebar.svelte';
import Timeline from '$lib/components/Timeline.svelte';
```

**Grid template:**
```css
.dashboard-grid {
  grid-template-columns: 2.5fr 1.5fr 1fr;  /* CHANGED */
}
```

**Footer height:**
```css
.dashboard-footer {
  height: 180px;  /* CHANGED from 250px */
}
```

## File Structure

```
visualizer/src/lib/components/
├── CompactSidebar.svelte  (NEW)     - Merged Events + Stops
├── Timeline.svelte        (NEW)     - Keyboard-friendly, no Chart.js
├── LinearRouteWidget.svelte (KEEP)  - No changes
├── MapView.svelte         (KEEP)    - No changes
├── ProbabilityScope.svelte (KEEP)   - No changes
├── FsmInspector.svelte    (KEEP)    - No changes
│
├── TimelineCharts.svelte  (DEPRECATED) - Replaced by Timeline.svelte
├── EventLog.svelte        (DEPRECATED) - Logic moved to CompactSidebar
└── AllStopsInspector.svelte (DEPRECATED) - Logic moved to CompactSidebar

visualizer/src/routes/
└── +page.svelte           (MODIFIED) - Grid layout, imports
```

## Constants and Helpers

### Event Type Labels

```typescript
function getEventTypeLabel(type: string): string {
  switch (type) {
    case 'ARRIVAL': return 'ARR';
    case 'TRANSITION': return 'TRN';
    case 'DEPARTURE': return 'DEP';
    default: return type.substring(0, 3);
  }
}
```

### Event Type Colors

```typescript
function getEventTypeColor(type: string): string {
  switch (type) {
    case 'ARRIVAL': return '#22c55e';
    case 'TRANSITION': return '#eab308';
    case 'DEPARTURE': return '#6b7280';
    default: return '#888';
  }
}
```

### Time Formatting

```typescript
function formatEventTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleTimeString([], {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }).substring(0, 8); // HH:MM:SS
}
```

## Acceptance Criteria

### Layout
- ✓ Map panel is 2.5fr (significantly larger than before)
- ✓ Lab panel is 1.5fr (unchanged size)
- ✓ Sidebar panel is 1fr (compact)
- ✓ Footer height is 180px (reduced from 250px)

### Compact Sidebar
- ✓ Events and Stops in 50/50 split view
- ✓ Section headers show event/stop counts
- ✓ Each section scrolls independently
- ✓ Current event highlighted in blue tint
- ✓ Active stop highlighted with distinct background

### All Stops (Compact)
- ✓ All original details visible: stop #, state, prob, distance, dwell, features
- ✓ Smaller fonts (0.6-0.7rem) and tighter spacing (0.4rem gaps)
- ✓ Click selects stop and shows details in Lab Panel
- ✓ Just arrived badge visible and animated

### Event Log (Compact)
- ✓ Inline format with alternating row colors (odd/even)
- ✓ Time, type badge, stop #, state all visible
- ✓ Current time event highlighted
- ✓ Click seeks playback and highlights on map
- ✓ Type badges use 3-char abbreviations (ARR/TRN/DEP)

### Timeline
- ✓ Play/pause button works
- ✓ Speed selector (1x/2x/5x/10x) works
- ✓ Time display shows current/total
- ✓ Seek slider works
- ✓ Keyboard shortcuts: Space (play/pause), Left/Right (±5s)
- ✓ Shortcuts hint visible: "␣ play ←→ seek ? help"
- ✓ Shortcuts ignored when typing in input fields

### Keyboard Shortcuts
- ✓ Space bar toggles play/pause
- ✓ Arrow Left seeks -5 seconds
- ✓ Arrow Right seeks +5 seconds
- ✓ Shortcuts don't trigger when user focuses input/select
- ✓ (Optional) ? key shows help modal

### Integration
- ✓ Selecting a stop in sidebar shows ProbabilityScope + FsmInspector in Lab Panel
- ✓ Clicking event seeks timeline and highlights on map
- ✓ Map still receives `highlightedEvent` for event markers
- ✓ LinearRouteWidget still displays bus position
- ✓ No functionality lost from original design

## Testing Checklist

1. Load route + trace data
2. Verify map is noticeably larger
3. Verify sidebar shows events (top) and stops (bottom)
4. Test clicking an event — verify timeline seeks and map highlights
5. Test clicking a stop — verify Lab Panel shows detailed view
6. Test Space key toggles play/pause
7. Test Arrow keys seek ±5 seconds
8. Verify current event is highlighted in event list
9. Verify selected stop is highlighted in stop list
10. Verify all stop details are visible (prob, distance, dwell, features)
11. Test playback with speed selector
12. Verify keyboard shortcuts don't trigger when focusing inputs
13. Test responsive behavior at narrow widths (< 1200px)

## Migration Notes

**Existing components deprecated but not deleted:**
- `TimelineCharts.svelte` → Keep for now, can delete after verification
- `EventLog.svelte` → Logic moved to CompactSidebar
- `AllStopsInspector.svelte` → Logic moved to CompactSidebar

**Rollback plan:** If issues arise, revert +page.svelte imports and grid CSS.

**Future enhancements (out of scope):**
- Help modal for ? key
- Timeline event markers (small dots for events)
- Responsive breakpoints for mobile
- Collapse/expand sidebar sections
