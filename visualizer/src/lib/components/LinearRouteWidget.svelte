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

	// Canvas dimensions - increased height for split layout
	let width = $state(0);
	let height = $state(120); // Increased from 80 to 120

	// Layout constants
	const LAYOUT = {
		topRowY: 35,        // Route line Y position
		bottomRowY: 75,     // Scale line Y position
		eventRowY: 100,     // Event info Y position
		paddingX: 30,       // Horizontal padding
		stopLabelOffset: 18 // Distance from line to stop number
	};

	// Calculate scale and max progress
	const maxProgress = $derived.by(() => {
		if (routeData.nodes.length === 0) return 100000; // 1km fallback
		return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
	});

	const scale = $derived.by(() => {
		const drawWidth = width - LAYOUT.paddingX * 2;
		return maxProgress > 0 ? drawWidth / maxProgress : 1;
	});

	// Convert progress (cm) to canvas X coordinate
	function progressToX(progressCm: number): number {
		return LAYOUT.paddingX + Math.min(progressCm, maxProgress) * scale;
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

		// Clear canvas
		context.clearRect(0, 0, width, height);

		// ========== TOP ROW: Route line with stops and bus ==========
		const routeY = LAYOUT.topRowY;

		// Draw base route line
		context.strokeStyle = '#4a5568';
		context.lineWidth = 3;
		context.lineCap = 'round';
		context.beginPath();
		context.moveTo(LAYOUT.paddingX, routeY);
		context.lineTo(width - LAYOUT.paddingX, routeY);
		context.stroke();

		// Draw current segment highlight (blue glow)
		if (busProgress > 0) {
			const segment = findSegment(busProgress);
			if (segment) {
				const x1 = progressToX(segment.start);
				const x2 = progressToX(segment.end);

				// Glow effect
				context.shadowColor = '#3b82f6';
				context.shadowBlur = 10;
				context.strokeStyle = 'rgba(59, 130, 246, 0.8)';
				context.lineWidth = 4;
				context.beginPath();
				context.moveTo(x1, routeY);
				context.lineTo(x2, routeY);
				context.stroke();
				context.shadowBlur = 0; // Reset shadow
			}
		}

		// Draw stop indicators
		routeData.stops.forEach((stop, index) => {
			const x = progressToX(stop.progress_cm);

			// Vertical tick mark
			context.strokeStyle = '#ef4444';
			context.lineWidth = 2;
			context.beginPath();
			context.moveTo(x, routeY - 5);
			context.lineTo(x, routeY + 5);
			context.stroke();

			// Stop number label - larger, clearer
			context.fillStyle = '#fff';
			context.font = 'bold 13px JetBrains Mono, Monaco, monospace';
			context.textAlign = 'center';
			context.textBaseline = 'bottom';
			context.fillText(`#${index}`, x, routeY - LAYOUT.stopLabelOffset);
		});

		// Draw bus position (emoji)
		const busX = progressToX(busProgress);
		context.font = '18px serif';
		context.textAlign = 'center';
		context.textBaseline = 'middle';
		context.fillText('🚌', busX, routeY);

		// ========== BOTTOM ROW: Distance scale ==========
		const scaleY = LAYOUT.bottomRowY;

		// Draw scale line
		context.strokeStyle = '#718096';
		context.lineWidth = 2;
		context.beginPath();
		context.moveTo(LAYOUT.paddingX, scaleY);
		context.lineTo(width - LAYOUT.paddingX, scaleY);
		context.stroke();

		// Draw scale ticks (every 500m for cleaner look)
		const scaleIntervalCm = 50000; // 500m
		for (let d = 0; d <= maxProgress; d += scaleIntervalCm) {
			const x = progressToX(d);

			// Tick mark
			context.strokeStyle = '#718096';
			context.lineWidth = 1;
			context.beginPath();
			context.moveTo(x, scaleY - 4);
			context.lineTo(x, scaleY + 4);
			context.stroke();

			// Distance label - larger font
			context.fillStyle = '#a0aec0';
			context.font = '12px JetBrains Mono, Monaco, monospace';
			context.textAlign = 'center';
			context.textBaseline = 'top';
			const km = (d / 100000).toFixed(1);
			context.fillText(`${km}km`, x, scaleY + 8);
		}

		// ========== EVENT ROW: Highlighted event info ==========
		if (highlightedEvent) {
			const stop = routeData.stops[highlightedEvent.stopIdx];
			if (stop) {
				const eventY = LAYOUT.eventRowY;
				const color = FSM_STATE_COLORS[highlightedEvent.state];

				// Draw connecting line from stop to event text
				const stopX = progressToX(stop.progress_cm);
				context.strokeStyle = color;
				context.lineWidth = 1;
				context.setLineDash([4, 4]);
				context.beginPath();
				context.moveTo(stopX, routeY + LAYOUT.stopLabelOffset);
				context.lineTo(stopX, eventY - 5);
				context.stroke();
				context.setLineDash([]); // Reset dash

				// Event indicator circle on route
				context.fillStyle = color;
				context.beginPath();
				context.arc(stopX, routeY, 6, 0, Math.PI * 2);
				context.fill();
				context.strokeStyle = '#fff';
				context.lineWidth = 2;
				context.stroke();

				// Event info text
				context.textAlign = 'center';
				context.textBaseline = 'top';

				// Stop number in color
				context.font = 'bold 14px JetBrains Mono, Monaco, monospace';
				context.fillStyle = color;
				context.fillText(`Stop #${highlightedEvent.stopIdx}`, stopX, eventY);

				// State below
				context.font = '12px JetBrains Mono, Monaco, monospace';
				context.fillStyle = '#e2e8f0';
				context.fillText(`→ ${highlightedEvent.state}`, stopX, eventY + 16);
			}
		}
	}

	// Re-render when dependencies change
	$effect(() => {
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
