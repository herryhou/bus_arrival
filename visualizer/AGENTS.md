# Visualizer

Web-based bus arrival trace visualizer (SvelteKit).

## Build

```bash
cd visualizer
npm install
npm run dev
```

## Key Files

- `src/routes/` - SvelteKit pages
- `src/lib/` - Shared components
- `static/` - Static assets

## Data Format

Consumes `trace_v2.jsonl` from pipeline.

## Root Docs

See project root `CLAUDE.md` for:
- Overall architecture
- Data format specs
- Pipeline integration
