# Static Visualizer Design

**Date**: 2026-03-25
**Status**: Draft
**Author**: Claude Code

## Overview

Convert the existing SvelteKit-based bus arrival visualizer into a self-contained static HTML file for demo/presentation use. Each route distribution consists of exactly 3 files that can be deployed anywhere or opened locally.

**Motivation**: Enable easy distribution of visualizations for presentations, demos, and sharing without requiring a Node.js server or file upload workflow.

## Goals

- Single self-contained HTML file with embedded CSS/JS
- 3-file distribution per route: `{route}.html`, `{route}.bin`, `{route}.jsonl`
- No file upload UI - auto-loads data on page load
- Friendly error handling for network issues
- Pure static deployment - works from any static host or locally

## Non-Goals

- Dynamic data loading at runtime
- Multiple dataset switching in single deployment
- Server-side rendering or API endpoints

## Architecture

### Current State

- SvelteKit app with file upload UI
- User manually selects `route_data.bin` and `trace.jsonl`
- Build outputs multi-file static site with code-split chunks

### Target State

```
dist/{route}/
├── {route}.html      (self-contained, ~150-200KB)
├── {route}.bin       (route data)
└── {route}.jsonl     (trace data)
```

Single HTML file contains:
- All application code (Svelte components compiled to JS)
- All application styles
- Inline references to local `.bin` and `.jsonl` files

### Build Pipeline

```
SvelteKit source code
  → Vite build (single-bundle config)
  → bundle.js + bundle.css
  → Node script inlines into HTML template
  → {route}.html
```

## Technical Approach

### Vite Configuration

Modify `vite.config.ts` to output single bundle:

```typescript
build: {
  rollupOptions: {
    output: {
      inlineDynamicImports: true,  // Single JS file
      manualChunks: undefined,     // Disable code splitting
    }
  }
}
```

### Build Script (`build-static.ts`)

Node.js script that:
1. Accepts route name as CLI argument: `npm run build -- ty225`
2. Copies `{route}.bin` and `{route}.jsonl` from `data/` directory
3. Runs Vite build
4. Reads generated `bundle.js` and `bundle.css`
5. Creates HTML file with inlined content
6. Outputs to `dist/{route}/`

### External Dependencies

**Loaded from CDN only**:
- MapLibre GL JS CSS
- MapLibre GL JS

All application code is inlined.

## Component Changes

### Modified Files

**`src/routes/+page.svelte`**:
1. Remove `showUpload` state
2. Remove file upload UI elements
3. Hardcode data URLs: `/{route}.bin`, `/{route}.jsonl`
4. Auto-load in `onMount()`
5. Remove "New Session" button
6. Add error view with retry button

### Unchanged Files

- All `src/lib/components/*.svelte` files (work as-is)
- All parsers (`routeData.ts`, `trace.ts`) - already support URL fetching
- Types and utilities

## Data Flow

```
Page Load
  ↓
onMount()
  ↓
loadRouteData('/{route}.bin')
loadTraceFile('/{route}.jsonl')
  ↓
Parse using existing parsers
  ↓
Set state → Dashboard renders
```

### Error Handling

On load failure:
- Display user-friendly error message
- Show "Retry" button that re-attempts fetch
- No technical error details exposed to user

## File Structure

### Source Structure

```
visualizer/
├── src/
│   ├── routes/
│   │   └── +page.svelte          (modified)
│   └── lib/
│       ├── components/           (unchanged)
│       ├── parsers/              (unchanged)
│       └── types.ts              (unchanged)
├── data/
│   ├── ty225.bin
│   ├── ty225.jsonl
│   └── ... (other routes)
├── build-static.ts               (new)
├── vite.config.ts                (modified)
└── package.json                  (modified - add build script)
```

### Output Structure (per route)

```
dist/ty225/
├── ty225.html
├── ty225.bin
└── ty225.jsonl
```

## Implementation Plan

See [implementation plan](../../plans/2026-03-25-static-visualizer-implementation.md) for detailed steps.

## Deployment

The output directory can be:
- Served from any static host (GitHub Pages, Netlify, S3)
- Opened locally with `python -m http.server` or similar
- Zipped and shared as standalone demo package

## Alternatives Considered

| Option | Description | Rejection Reason |
|--------|-------------|------------------|
| Multi-file SvelteKit build | Keep default code-splitting | Doesn't meet single-file requirement |
| Base64 embedded data | Embed bin/jsonl in HTML | File sizes too large for embedding |
| Vanilla JS rewrite | Remove Svelte entirely | Unnecessary complexity, Svelte works fine |

## Open Questions

None.
