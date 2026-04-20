# Bus Arrival Visualizer

Interactive visualization tool for bus arrival detection system, displaying GPS traces, route data, and arrival probabilities in real-time.

## Features

- **Smart File Loading**: When using Chrome/Edge, selecting a `.bin` file auto-discovers matching `_trace.jsonl` in the same directory
- **Probability Visualization**: Linear route view shows color-coded probabilities for all stops
- **Detailed Tooltips**: Hover over any stop to see full probability breakdown with feature scores
- **FSM State Badges**: Visual indicators for state transitions (Approaching → Arriving → AtStop → Departed)

## Browser Compatibility

| Feature | Chrome/Edge | Firefox | Safari |
|---------|-------------|---------|--------|
| Smart file loading | ✓ | ✗ | ✗ |
| Traditional upload | ✓ | ✓ | ✓ |
| Probability viz | ✓ | ✓ | ✓ |

## Usage

1. Click "Select Route Data (.bin)" or use traditional file inputs
2. If using Chrome/Edge, matching trace file is auto-discovered
3. Scrub timeline to see probabilities update in real-time
4. Hover stops for detailed breakdown
5. Click stops to select and view in Lab panel

## Development

Once you've created a project and installed dependencies with `npm install` (or `pnpm install` or `yarn`), start a development server:

```sh
npm run dev

# or start the server and open the app in a new browser tab
npm run dev -- --open
```

## Building

To create a production version of your app:

```sh
npm run build
```

You can preview the production build with `npm run preview`.

> To deploy your app, you may need to install an [adapter](https://svelte.dev/docs/kit/adapters) for your target environment.
