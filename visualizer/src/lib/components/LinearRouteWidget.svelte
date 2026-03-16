<!-- visualizer/src/lib/components/LinearRouteWidget.svelte -->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { RouteData, FsmState } from '$lib/types';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';

	interface Props {
		routeData: RouteData;
		busProgress: number; // Current s_cm from TraceRecord
		highlightedEvent?: {
			stopIdx: number;
			state: FsmState;
		} | null;
	}

	let { routeData, busProgress: busProgressProp, highlightedEvent = null }: Props = $props();

	// Make props reactive by storing them in local state
	let busProgress = $derived.by(() => busProgressProp);

	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D | null = null;
	let resizeObserver: ResizeObserver | null = null;

	// Canvas dimensions
	let width = $state(0);
	let height = $state(80);

	// Constants for rendering
	const PADDING = { left: 20, right: 20 };
	const TICK_INTERVAL_M = 100; // 100m
	const MAJOR_TICK_INTERVAL_M = 1000; // 1km
	const TICK_INTERVAL_CM = TICK_INTERVAL_M * 100; // 10,000 cm
	const MAJOR_TICK_INTERVAL_CM = MAJOR_TICK_INTERVAL_M * 100; // 100,000 cm

	// Calculate scale and max progress
	const maxProgress = $derived.by(() => {
		if (routeData.nodes.length === 0) return 100000; // 1km fallback
		return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
	});

	const scale = $derived.by(() => {
		const drawWidth = width - PADDING.left - PADDING.right;
		return maxProgress > 0 ? drawWidth / maxProgress : 1;
	});

	// Convert progress (cm) to canvas X coordinate
	function progressToX(progressCm: number): number {
		return PADDING.left + Math.min(progressCm, maxProgress) * scale;
	}

	// Find current segment based on bus progress
	function findSegment(progressCm: number): { start: number; end: number } | null {
		if (routeData.nodes.length < 2) return null;
		if (progressCm < routeData.nodes[0].cum_dist_cm) return null;
		if (progressCm >= routeData.nodes[routeData.nodes.length - 1].cum_dist_cm) {
			const lastIdx = routeData.nodes.length - 2;
			return {
				start: routeData.nodes[lastIdx].cum_dist_cm,
				end: routeData.nodes[lastIdx + 1].cum_dist_cm
			};
		}

		for (let i = 0; i < routeData.nodes.length - 1; i++) {
			const node = routeData.nodes[i];
			const nextNode = routeData.nodes[i + 1];
			if (progressCm >= node.cum_dist_cm && progressCm < nextNode.cum_dist_cm) {
				return { start: node.cum_dist_cm, end: nextNode.cum_dist_cm };
			}
		}
		return null;
	}

	// Render function
	function render() {
		if (!ctx || width === 0) return;

		const context = ctx;
		const centerY = height / 2;

		// Clear canvas
		context.clearRect(0, 0, width, height);

		// 1. Draw base line
		context.strokeStyle = '#333';
		context.lineWidth = 4;
		context.beginPath();
		context.moveTo(PADDING.left, centerY);
		context.lineTo(width - PADDING.right, centerY);
		context.stroke();

		// 2. Draw distance ticks
		for (let d = 0; d <= maxProgress; d += TICK_INTERVAL_CM) {
			const x = progressToX(d);
			const isMajor = d % MAJOR_TICK_INTERVAL_CM === 0;
			const tickHeight = isMajor ? 8 : 4;

			context.strokeStyle = '#555';
			context.lineWidth = 1;
			context.beginPath();
			context.moveTo(x, centerY - tickHeight / 2);
			context.lineTo(x, centerY + tickHeight / 2);
			context.stroke();

			if (isMajor) {
				context.fillStyle = '#666';
				context.font = '10px JetBrains Mono, Monaco, monospace';
				context.textAlign = 'center';
				context.fillText(`${(d / 100000).toFixed(1)}km`, x, centerY - 12);
			}
		}

		// 3. Draw stop indicators
		routeData.stops.forEach((stop, index) => {
			const x = progressToX(stop.progress_cm);

			// Vertical line
			context.strokeStyle = '#ef4444';
			context.lineWidth = 2;
			context.beginPath();
			context.moveTo(x, centerY - 6);
			context.lineTo(x, centerY + 6);
			context.stroke();

			// Stop number label
			context.fillStyle = '#ef4444';
			context.font = '11px JetBrains Mono, Monaco, monospace';
			context.textAlign = 'center';
			context.fillText(index.toString(), x, centerY - 10);
		});

		// 4. Draw current segment highlight
		if (busProgress > 0) {
			const segment = findSegment(busProgress);
			if (segment) {
				const x1 = progressToX(segment.start);
				const x2 = progressToX(segment.end);

				context.fillStyle = 'rgba(59, 130, 246, 0.2)';
				context.fillRect(x1, centerY - 10, x2 - x1, 20);
			}
		}

		// 5. Draw bus position
		const busX = progressToX(busProgress);
		context.font = '16px serif';
		context.textAlign = 'center';
		context.textBaseline = 'middle';
		context.fillText('🚌', busX, centerY);

		// 6. Draw event highlight if present
		if (highlightedEvent) {
			const stop = routeData.stops[highlightedEvent.stopIdx];
			if (stop) {
				const x = progressToX(stop.progress_cm);
				const color = FSM_STATE_COLORS[highlightedEvent.state];

				// Vertical marker
				context.strokeStyle = color;
				context.lineWidth = 3;
				context.beginPath();
				context.moveTo(x, centerY - 15);
				context.lineTo(x, centerY + 15);
				context.stroke();

				// Color dot
				context.fillStyle = color;
				context.beginPath();
				context.arc(x, centerY - 20, 5, 0, Math.PI * 2);
				context.fill();
			}
		}
	}

	// Re-render when dependencies change
	$effect(() => {
		// Track reactive dependencies
		busProgress;
		routeData;
		highlightedEvent;
		render();
	});

	// Handle canvas resize
	onMount(() => {
		ctx = canvas.getContext('2d');
		if (!ctx) return;

		// Initial size
		updateSize();

		// Observe resize
		resizeObserver = new ResizeObserver(() => {
			updateSize();
		});
		resizeObserver.observe(canvas);

		return () => {
			resizeObserver?.disconnect();
		};
	});

	function updateSize() {
		const rect = canvas.getBoundingClientRect();
		width = rect.width;
		height = rect.height;

		// Update canvas resolution for high-DPI displays
		const dpr = window.devicePixelRatio || 1;
		canvas.width = rect.width * dpr;
		canvas.height = rect.height * dpr;

		if (ctx) {
			ctx.scale(dpr, dpr);
		}

		render();
	}

	onDestroy(() => {
		resizeObserver?.disconnect();
	});
</script>

<div class="linear-route-widget">
	<canvas bind:this={canvas}></canvas>
</div>

<style>
	.linear-route-widget {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	canvas {
		width: 100%;
		height: 100%;
		display: block;
	}
</style>
