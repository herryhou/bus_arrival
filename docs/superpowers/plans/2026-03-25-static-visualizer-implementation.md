# Static Visualizer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the SvelteKit visualizer into a self-contained static HTML distribution that auto-loads a fixed dataset.

**Architecture:** Modify the Vite build to output a single JS bundle, create a Node.js build script that inlines CSS/JS into an HTML template, and update the Svelte component to auto-load data from fixed URLs.

**Tech Stack:** SvelteKit, Vite, Node.js (fs/path), TypeScript

---

## File Structure

**Create:**
- `visualizer/build-static.ts` - Build script that orchestrates the single-file HTML generation
- `visualizer/data/` - Directory containing symlinks to source data files

**Modify:**
- `visualizer/vite.config.ts` - Add single-bundle rollup configuration
- `visualizer/src/routes/+page.svelte` - Remove upload UI, add auto-load with placeholder for build-time replacement
- `visualizer/package.json` - Add build script command

**Source Data Files** (already exist in `visualizer/static/`):
- `ty225.bin`, `ty225_trace.jsonl` - Primary test dataset
- `downtown.bin`, `downtown_trace.jsonl` - Additional dataset
- `normal.bin`, `normal_trace.jsonl` - Additional dataset

**Output Structure** (per route):
```
dist/ty225/
├── ty225.html
├── ty225.bin
└── ty225_trace.jsonl
```

---

## Task 1: Create data directory with symlinks

**Files:**
- Create: `visualizer/data/ty225.bin` (symlink)
- Create: `visualizer/data/ty225_trace.jsonl` (symlink)

- [ ] **Step 1: Create data directory**

Run: `mkdir -p /workspace/visualizer/data`
Expected: Directory created

- [ ] **Step 2: Create symlink for ty225.bin**

Run: `cd /workspace/visualizer/data && ln -s ../static/ty225.bin ty225.bin`
Expected: Symlink created

- [ ] **Step 3: Create symlink for ty225_trace.jsonl**

Run: `cd /workspace/visualizer/data && ln -s ../static/ty225_trace.jsonl ty225_trace.jsonl`
Expected: Symlink created

- [ ] **Step 4: Verify symlinks**

Run: `ls -la /workspace/visualizer/data/`
Expected: Shows ty225.bin and ty225_trace.jsonl as symlinks

- [ ] **Step 5: Commit**

```bash
git add visualizer/data
git commit -m "feat: add data directory with symlinks to source files"
```

---

## Task 2: Configure Vite for single-bundle output

**Files:**
- Modify: `visualizer/vite.config.ts`

- [ ] **Step 1: Add rollup configuration for single bundle**

Edit `/workspace/visualizer/vite.config.ts` to add build configuration:

```typescript
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	build: {
		rollupOptions: {
			output: {
				inlineDynamicImports: true,  // Single JS file
				manualChunks: undefined,     // Disable code splitting
			}
		}
	}
});
```

- [ ] **Step 2: Verify TypeScript syntax**

Run: `cd /workspace/visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/vite.config.ts
git commit -m "build: configure Vite for single-bundle output"
```

---

## Task 3: Modify +page.svelte for auto-load

**Files:**
- Modify: `visualizer/src/routes/+page.svelte:37-96` (remove upload UI)
- Modify: `visualizer/src/routes/+page.svelte:13-15` (add route constant)
- Modify: `visualizer/src/routes/+page.svelte:41-59` (add auto-load in onMount)
- Modify: `visualizer/src/routes/+page.svelte:139-145` (remove resetUpload)
- Modify: `visualizer/src/routes/+page.svelte:164-194` (remove upload screen markup)
- Modify: `visualizer/src/routes/+page.svelte:204` (remove New Session button)

- [ ] **Step 1: Add ROUTE_NAME constant**

After line 12 (after `let traceData = $state<TraceData | null>(null);`), add:

```typescript
// Build-time placeholder - replaced by build script
const ROUTE_NAME = '__ROUTE_NAME__';
```

- [ ] **Step 2: Remove upload-related state**

Delete these lines (37-39):
```typescript
let showUpload = $state(true);
let loading = $state(false);
let error = $state<string | null>(null);
```

Replace with:
```typescript
let loading = $state(false);
let error = $state<string | null>(null);
```

- [ ] **Step 3: Modify onMount to auto-load**

Replace the entire `onMount` function (lines 42-59) with:

```typescript
onMount(async () => {
	// Auto-load data on mount
	await loadAllData();

	// Start playback timer
	const interval = setInterval(() => {
		if (isPlaying && traceData && currentTime < timeMax) {
			const fps = 10; // Base updates per second
			const dt = (1 / fps) * playbackSpeed;
			const nextTime = currentTime + dt;
			if (nextTime >= timeMax) {
				currentTime = timeMax;
				isPlaying = false;
			} else {
				currentTime = nextTime;
			}
			currentTimePercent = ((currentTime - timeMin) / (timeMax - timeMin)) * 100;
		}
	}, 100);

	return () => clearInterval(interval);
});
```

- [ ] **Step 4: Add loadAllData function**

Insert after the `onMount` function:

```typescript
async function loadAllData() {
	loading = true;
	error = null;
	try {
		// Use build-time replaced route name
		const routeUrl = `/${ROUTE_NAME}.bin`;
		const traceUrl = `/${ROUTE_NAME}_trace.jsonl`;

		[routeData] = await Promise.all([
			loadRouteData(routeUrl),
			loadTraceFile(traceUrl).then(data => {
				[timeMin, timeMax] = getTraceTimeRange(data);
				currentTime = timeMin;
				currentTimePercent = 0;
				return data;
			})
		]);
	} catch (e) {
		error = `Failed to load data: ${e instanceof Error ? e.message : String(e)}`;
	} finally {
		loading = false;
	}
}
```

- [ ] **Step 5: Remove upload handler functions**

Delete `handleRouteUpload` (lines 61-73) and `handleTraceUpload` (lines 75-90) functions.

- [ ] **Step 6: Remove checkReady function**

Delete the `checkReady` function (lines 92-96).

- [ ] **Step 7: Remove resetUpload function**

Delete the `resetUpload` function (lines 139-145).

- [ ] **Step 8: Remove upload screen markup**

Delete the entire `{#if showUpload}` block (lines 164-194).

- [ ] **Step 9: Remove New Session button**

Delete line 204: `<button onclick={resetUpload} class="btn-outline">New Session</button>`

- [ ] **Step 10: Add error handling UI**

Add after the `<header>` section (after line 206), before `<main>`:

```svelte
{#if error}
<div class="error-banner" style="position: fixed; top: 60px; left: 50%; transform: translateX(-50%); background: #dc2626; color: white; padding: 1rem 2rem; border-radius: 0.5rem; z-index: 1000; text-align: center;">
	<div style="margin-bottom: 0.5rem;">{error}</div>
	<button onclick={loadAllData} style="background: white; color: #dc2626; border: none; padding: 0.5rem 1rem; border-radius: 0.25rem; cursor: pointer; font-weight: bold;">Retry</button>
</div>
{/if}

{#if loading}
<div style="position: fixed; top: 60px; left: 50%; transform: translateX(-50%); background: #1a1a1a; color: #3b82f6; padding: 1rem 2rem; border-radius: 0.5rem; z-index: 1000;">Loading data...</div>
{/if}
```

- [ ] **Step 11: Verify TypeScript**

Run: `cd /workspace/visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 12: Test dev build**

Run: `cd /workspace/visualizer && npm run build`
Expected: Build succeeds

- [ ] **Step 13: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat: remove upload UI, add auto-load with build-time placeholder"
```

---

## Task 4: Create build script

**Files:**
- Create: `visualizer/build-static.ts`

- [ ] **Step 1: Create build script file**

Create `/workspace/visualizer/build-static.ts`:

```typescript
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';

const ROUTE_NAME = process.argv[2];
if (!ROUTE_NAME) {
	console.error('Usage: npm run build:static <route_name>');
	console.error('Example: npm run build:static ty225');
	process.exit(1);
}

const DATA_DIR = path.resolve('data');
const BUILD_DIR = path.resolve('.svelte-kit/output/client');
const DIST_DIR = path.resolve(`dist/${ROUTE_NAME}`);
const SOURCE_BIN = path.join(DATA_DIR, `${ROUTE_NAME}.bin`);
const SOURCE_JSONL = path.join(DATA_DIR, `${ROUTE_NAME}_trace.jsonl`);

// Verify source files exist
if (!fs.existsSync(SOURCE_BIN)) {
	console.error(`Source file not found: ${SOURCE_BIN}`);
	process.exit(1);
}
if (!fs.existsSync(SOURCE_JSONL)) {
	console.error(`Source file not found: ${SOURCE_JSONL}`);
	process.exit(1);
}

console.log(`Building static visualizer for route: ${ROUTE_NAME}`);

// Clean and create dist directory
fs.rmSync(DIST_DIR, { recursive: true, force: true });
fs.mkdirSync(DIST_DIR, { recursive: true });

// Copy data files to dist
fs.copyFileSync(SOURCE_BIN, path.join(DIST_DIR, `${ROUTE_NAME}.bin`));
fs.copyFileSync(SOURCE_JSONL, path.join(DIST_DIR, `${ROUTE_NAME}_trace.jsonl`));
console.log('✓ Copied data files');

// Build the app (single bundle)
console.log('Building SvelteKit app...');
try {
	execSync('npm run build', { stdio: 'inherit' });
} catch (e) {
	console.error('Build failed');
	process.exit(1);
}
console.log('✓ SvelteKit build complete');

// Find the generated bundle files
const buildDir = path.resolve('build/_app/immutable');
let bundleJs: string | null = null;
let bundleCss: string | null = null;

if (fs.existsSync(buildDir)) {
	const files = fs.readdirSync(buildDir, { recursive: true }) as string[];
	for (const file of files) {
		const fullPath = path.join(buildDir, file);
		if (file.endsWith('.js') && !fullPath.includes('chunks')) {
			bundleJs = fullPath;
		}
		if (file.endsWith('.css')) {
			bundleCss = fullPath;
		}
	}
}

if (!bundleJs) {
	console.error('Could not find bundle JS file');
	process.exit(1);
}

// Read bundle content
const jsContent = fs.readFileSync(bundleJs, 'utf-8');
const cssContent = bundleCss ? fs.readFileSync(bundleCss, 'utf-8') : '';

// Replace ROUTE_NAME placeholder
const finalJs = jsContent.replace(/__ROUTE_NAME__/g, ROUTE_NAME);

// Generate HTML
const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>${ROUTE_NAME} - Bus Arrival Lab</title>
  <link href="https://unpkg.com/maplibre-gl@4.1.0/dist/maplibre-gl.css" rel="stylesheet" />
  <style>
${cssContent}
  </style>
</head>
<body class="dark">
  <div style="display: contents" id="app"></div>
  <script src="https://unpkg.com/maplibre-gl@4.1.0/dist/maplibre-gl.js"></script>
  <script>
${finalJs}
  </script>
</body>
</html>
`;

// Write HTML file
fs.writeFileSync(path.join(DIST_DIR, `${ROUTE_NAME}.html`), html);
console.log(`✓ Generated ${ROUTE_NAME}.html`);

console.log(`\n✨ Build complete! Output: ${DIST_DIR}`);
console.log(`   Files: ${ROUTE_NAME}.html, ${ROUTE_NAME}.bin, ${ROUTE_NAME}_trace.jsonl`);
```

- [ ] **Step 2: Add shebang and make executable**

Add to top of file:
```typescript
#!/usr/bin/env -S node --loader ts-node/esm
```

Run: `chmod +x /workspace/visualizer/build-static.ts`
Expected: File is executable

- [ ] **Step 3: Verify TypeScript**

Run: `cd /workspace/visualizer && npx tsc --noEmit build-static.ts`
Expected: No TypeScript errors

- [ ] **Step 4: Commit**

```bash
git add visualizer/build-static.ts
git commit -m "feat: add build script for single-file HTML generation"
```

---

## Task 5: Update package.json scripts

**Files:**
- Modify: `visualizer/package.json`

- [ ] **Step 1: Add build:static script**

Add to `scripts` section (after line 9):

```json
"build:static": "ts-node build-static.ts"
```

- [ ] **Step 2: Install ts-node dependency**

Run: `cd /workspace/visualizer && npm install --save-dev ts-node`
Expected: ts-node installed

- [ ] **Step 3: Verify package.json syntax**

Run: `cd /workspace/visualizer && cat package.json | jq .`
Expected: Valid JSON

- [ ] **Step 4: Commit**

```bash
git add visualizer/package.json visualizer/package-lock.json
git commit -m "build: add build:static script"
```

---

## Task 6: Test build with ty225 route

**Files:**
- Test: `dist/ty225/` output

- [ ] **Step 1: Run build for ty225**

Run: `cd /workspace/visualizer && npm run build:static ty225`
Expected: Build completes with "✨ Build complete!" message

- [ ] **Step 2: Verify output files exist**

Run: `ls -la /workspace/visualizer/dist/ty225/`
Expected: Shows `ty225.html`, `ty225.bin`, `ty225_trace.jsonl`

- [ ] **Step 3: Check HTML file size**

Run: `wc -c /workspace/visualizer/dist/ty225/ty225.html`
Expected: File is 150-300KB (contains inlined CSS/JS)

- [ ] **Step 4: Verify ROUTE_NAME replacement**

Run: `grep -o "ty225" /workspace/visualizer/dist/ty225/ty225.html | head -5`
Expected: Shows multiple occurrences (route name was replaced)

- [ ] **Step 5: Test local server**

Run: `cd /workspace/visualizer/dist/ty225 && python3 -m http.server 8080 &`
Expected: Server starts on port 8080

- [ ] **Step 6: Open in browser (manual test)**

Open: http://localhost:8080/ty225.html
Expected: Page loads, map displays, data auto-loads

- [ ] **Step 7: Test error handling**

Stop the server, move the bin file temporarily, and reload:
```bash
mv /workspace/visualizer/dist/ty225/ty225.bin /workspace/visualizer/dist/ty225/ty225.bin.bak
# Reload browser - should show error
mv /workspace/visualizer/dist/ty225/ty225.bin.bak /workspace/visualizer/dist/ty225/ty225.bin
```
Expected: Error banner appears with retry button

- [ ] **Step 8: Kill test server**

Run: `pkill -f "python3 -m http.server"`
Expected: Server stopped

- [ ] **Step 9: Test direct file opening**

Open: `file:///workspace/visualizer/dist/ty225/ty225.html` in browser
Expected: Works without server (may have CORS issues with some browsers)

- [ ] **Step 10: Commit dist directory to git (optional)**

```bash
git add visualizer/dist/ty225/
git commit -m "test: add ty225 static build output"
```

---

## Task 7: Add .gitignore for dist directory

**Files:**
- Modify: `visualizer/.gitignore`

- [ ] **Step 1: Add dist to gitignore**

Add to `/workspace/visualizer/.gitignore`:
```
dist/
```

- [ ] **Step 2: Verify existing dist is ignored**

Run: `cd /workspace/visualizer && git status`
Expected: dist/ty225/ shows as ignored if not already committed

- [ ] **Step 3: Commit**

```bash
git add visualizer/.gitignore
git commit -m "chore: ignore dist directory"
```

---

## Task 8: Build additional routes (optional validation)

**Files:**
- Test: `dist/downtown/`, `dist/normal/`

- [ ] **Step 1: Create symlinks for downtown**

Run: `cd /workspace/visualizer/data && ln -s ../static/downtown.bin downtown.bin && ln -s ../static/downtown_trace.jsonl downtown_trace.jsonl`
Expected: Symlinks created

- [ ] **Step 2: Build downtown route**

Run: `cd /workspace/visualizer && npm run build:static downtown`
Expected: Build completes successfully

- [ ] **Step 3: Verify downtown output**

Run: `ls -la /workspace/visualizer/dist/downtown/`
Expected: Shows `downtown.html`, `downtown.bin`, `downtown_trace.jsonl`

- [ ] **Step 4: Test downtown in browser**

Open: `file:///workspace/visualizer/dist/downtown/downtown.html` in browser
Expected: Page loads with downtown data

- [ ] **Step 5: Create symlinks for normal**

Run: `cd /workspace/visualizer/data && ln -s ../static/normal.bin normal.bin && ln -s ../static/normal_trace.jsonl normal_trace.jsonl`
Expected: Symlinks created

- [ ] **Step 6: Build normal route**

Run: `cd /workspace/visualizer && npm run build:static normal`
Expected: Build completes successfully

- [ ] **Step 7: Commit data directory updates**

```bash
git add visualizer/data
git commit -m "feat: add symlinks for downtown and normal routes"
```

---

## Task 9: Update README with usage instructions

**Files:**
- Modify: `visualizer/README.md`

- [ ] **Step 1: Add usage section**

Add to `/workspace/visualizer/README.md`:

```markdown
## Static Build Distribution

Create self-contained HTML visualizations for demo/presentation use.

### Building a Route

```bash
npm run build:static <route_name>
```

Example:
```bash
npm run build:static ty225
```

This creates `dist/<route_name>/` with three files:
- `<route_name>.html` - Self-contained HTML with embedded CSS/JS
- `<route_name>.bin` - Route data
- `<route_name>_trace.jsonl` - Trace data

### Adding a New Route

1. Place data files in `visualizer/static/`:
   - `<route>.bin`
   - `<route>_trace.jsonl`

2. Create symlinks in `visualizer/data/`:
   ```bash
   cd visualizer/data
   ln -s ../static/<route>.bin <route>.bin
   ln -s ../static/<route>_trace.jsonl <route>_trace.jsonl
   ```

3. Build:
   ```bash
   npm run build:static <route>
   ```

### Deployment

The output directory can be:
- Served from any static host (GitHub Pages, Netlify, S3)
- Opened locally with `python -m http.server`
- Zipped and shared as a standalone demo
```

- [ ] **Step 2: Commit**

```bash
git add visualizer/README.md
git commit -m "docs: add static build usage instructions"
```

---

## Summary

This implementation:
1. Creates a `data/` directory with symlinks to source data files
2. Configures Vite for single-bundle output
3. Modifies the main component to auto-load data without upload UI
4. Creates a build script that generates self-contained HTML files
5. Adds npm scripts for easy building

**Result:** Each route becomes a portable 3-file package that can be deployed anywhere.
