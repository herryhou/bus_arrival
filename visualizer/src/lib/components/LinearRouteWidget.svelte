<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { RouteData, FsmState, Stop } from '$lib/types';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';

	interface Props {
		routeData: RouteData;
		busProgress: number;
		busSpeed?: number; // cm/s
		highlightedEvent?: {
			stopIdx: number;
			state: FsmState;
		} | null;
	}

	let { routeData, busProgress: busProgressProp, busSpeed = 0, highlightedEvent = null }: Props = $props();
	let busProgress = $derived.by(() => busProgressProp);

	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D | null = null;
	let resizeObserver: ResizeObserver | null = null;

	let width = $state(0);
	let height = $state(200);

	const LAYOUT = {
		topRowY: 45,
		scaleRowY: 85,
		detailPanelY: 125, // Start of detail panel
		paddingX: 30,
		stopLabelOffset: 20,
	};

	// Find stops within a certain distance of current position
	const nearbyStops = $derived.by(() => {
		const SEARCH_RADIUS_CM = 100000; // 1km
		return routeData.stops
			.map((stop, index) => ({
				stop,
				index,
				distance: Math.abs(stop.progress_cm - busProgress)
			}))
			.filter(({ distance }) => distance <= SEARCH_RADIUS_CM)
			.sort((a, b) => a.distance - b.distance)
			.slice(0, 3); // Show up to 3 nearby stops
	});

	// Calculate speed in km/h
	const speedKmh = $derived.by(() => {
		return (busSpeed * 3600) / 100000; // cm/s to km/h
	});

	const maxProgress = $derived.by(() => {
		if (routeData.nodes.length === 0) return 100000;
		return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
	});

	const scale = $derived.by(() => {
		const drawWidth = width - LAYOUT.paddingX * 2;
		return maxProgress > 0 ? drawWidth / maxProgress : 1;
	});

	function progressToX(progressCm: number): number {
		return LAYOUT.paddingX + Math.min(progressCm, maxProgress) * scale;
	}

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

	function render() {
		if (!ctx || width === 0) return;

		const context = ctx;

		// Clear canvas
		context.clearRect(0, 0, width, height);

		// ========== TOP ROW: Route line ==========
		const routeY = LAYOUT.topRowY;

		// Draw base line
		context.strokeStyle = '#4a5568';
		context.lineWidth = 3;
		context.lineCap = 'round';
		context.beginPath();
		context.moveTo(LAYOUT.paddingX, routeY);
		context.lineTo(width - LAYOUT.paddingX, routeY);
		context.stroke();

		// Current segment highlight
		if (busProgress > 0) {
			const segment = findSegment(busProgress);
			if (segment) {
				const x1 = progressToX(segment.start);
				const x2 = progressToX(segment.end);

				context.shadowColor = '#3b82f6';
				context.shadowBlur = 10;
				context.strokeStyle = 'rgba(59, 130, 246, 0.8)';
				context.lineWidth = 5;
				context.beginPath();
				context.moveTo(x1, routeY);
				context.lineTo(x2, routeY);
				context.stroke();
				context.shadowBlur = 0;
			}
		}

		// Draw stops
		routeData.stops.forEach((stop, index) => {
			const x = progressToX(stop.progress_cm);
			const isHighlighted = highlightedEvent && highlightedEvent.stopIdx === index;

			// Tick mark
			context.strokeStyle = '#ef4444';
			context.lineWidth = 2;
			context.beginPath();
			context.moveTo(x, routeY - 6);
			context.lineTo(x, routeY + 6);
			context.stroke();

			// Stop number
			context.fillStyle = isHighlighted ? '#f59e0b' : '#ffffff';
			context.font = 'bold 15px JetBrains Mono, Monaco, monospace';
			context.textAlign = 'center';
			context.textBaseline = 'bottom';
			context.fillText(`#${index + 1}`, x, routeY - LAYOUT.stopLabelOffset);

			// Highlight ring
			if (isHighlighted && highlightedEvent) {
				context.fillStyle = FSM_STATE_COLORS[highlightedEvent.state];
				context.beginPath();
				context.arc(x, routeY, 7, 0, Math.PI * 2);
				context.fill();
				context.strokeStyle = '#ffffff';
				context.lineWidth = 2;
				context.stroke();
			}
		});

		// Bus emoji
		const busX = progressToX(busProgress);
		context.font = '20px serif';
		context.textAlign = 'center';
		context.textBaseline = 'middle';
		context.fillText('🚌', busX, routeY);

		// ========== MIDDLE ROW: Scale ==========
		const scaleY = LAYOUT.scaleRowY;

		// Scale line
		context.strokeStyle = '#718096';
		context.lineWidth = 2;
		context.beginPath();
		context.moveTo(LAYOUT.paddingX, scaleY);
		context.lineTo(width - LAYOUT.paddingX, scaleY);
		context.stroke();

		// Scale markers (1.5km intervals - 5x the previous 500m)
		const intervalCm = 150000; // 2.5km
		for (let d = 0; d <= maxProgress; d += intervalCm) {
			const x = progressToX(d);

			context.strokeStyle = '#718096';
			context.lineWidth = 1;
			context.beginPath();
			context.moveTo(x, scaleY - 4);
			context.lineTo(x, scaleY + 4);
			context.stroke();

			context.fillStyle = '#cbd5e0';
			context.font = '13px JetBrains Mono, Monaco, monospace';
			context.textAlign = 'center';
			context.textBaseline = 'top';
			const km = (d / 100000).toFixed(1);
			context.fillText(`${km}km`, x, scaleY + 8);
		}

		// ========== BOTTOM ROW: Event info ==========
		if (highlightedEvent) {
			const stop = routeData.stops[highlightedEvent.stopIdx];
			if (stop) {
				const eventY = 105;
				const x = progressToX(stop.progress_cm);
				const color = FSM_STATE_COLORS[highlightedEvent.state];

				// Connecting line
				context.strokeStyle = color;
				context.lineWidth = 1;
				context.setLineDash([4, 4]);
				context.beginPath();
				context.moveTo(x, routeY + LAYOUT.stopLabelOffset);
				context.lineTo(x, eventY - 8);
				context.stroke();
				context.setLineDash([]);

				// Event text
				context.textAlign = 'center';
				context.textBaseline = 'top';

				// Stop number
				context.font = 'bold 15px JetBrains Mono, Monaco, monospace';
				context.fillStyle = color;
				context.fillText(`Stop #${highlightedEvent.stopIdx + 1}`, x, eventY);

				// State
				context.font = '14px JetBrains Mono, Monaco, monospace';
				context.fillStyle = '#e2e8f0';
				context.fillText(`→ ${highlightedEvent.state}`, x, eventY + 16);
			}
		}

		// ========== DETAIL PANEL ==========
		const detailY = LAYOUT.detailPanelY;

		// Panel background
		context.fillStyle = 'rgba(30, 41, 59, 0.8)';
		roundRect(context, LAYOUT.paddingX - 10, detailY - 8, width - LAYOUT.paddingX * 2 + 20, 65, 8);
		context.fill();

		// Speed indicator (left side)
		context.textAlign = 'left';
		context.textBaseline = 'top';

		context.font = '12px JetBrains Mono, Monaco, monospace';
		context.fillStyle = '#94a3b8';
		context.fillText('SPEED', LAYOUT.paddingX, detailY + 4);

		context.font = 'bold 24px JetBrains Mono, Monaco, monospace';
		context.fillStyle = speedKmh > 0 ? '#22c55e' : '#64748b';
		context.fillText(`${speedKmh.toFixed(1)} km/h`, LAYOUT.paddingX, detailY + 20);

		// Progress info (center)
		const progressKm = (busProgress / 100000).toFixed(2);
		const totalKm = (maxProgress / 100000).toFixed(1);
		const progressPercent = ((busProgress / maxProgress) * 100).toFixed(0);

		context.textAlign = 'center';
		context.font = '12px JetBrains Mono, Monaco, monospace';
		context.fillStyle = '#94a3b8';
		context.fillText('PROGRESS', width / 2, detailY + 4);

		context.font = 'bold 16px JetBrains Mono, Monaco, monospace';
		context.fillStyle = '#e2e8f0';
		context.fillText(`${progressKm} / ${totalKm} km`, width / 2, detailY + 20);

		context.font = '13px JetBrains Mono, Monaco, monospace';
		context.fillStyle = '#f59e0b';
		context.fillText(`${progressPercent}%`, width / 2, detailY + 40);

		// Nearby stops (right side)
		if (nearbyStops.length > 0) {
			const rightX = width - LAYOUT.paddingX;

			context.textAlign = 'right';
			context.font = '12px JetBrains Mono, Monaco, monospace';
			context.fillStyle = '#94a3b8';
			context.fillText('NEARBY STOPS', rightX, detailY + 4);

			nearbyStops.forEach(({ stop, index, distance }, i) => {
				const y = detailY + 20 + i * 14;
				const distanceKm = (distance / 100000).toFixed(2);
				const direction = stop.progress_cm > busProgress ? '→' : '←';

				context.font = '12px JetBrains Mono, Monaco, monospace';
				context.fillStyle = distance < 50000 ? '#22c55e' : '#cbd5e0'; // Green if < 500m
				context.fillText(`#${index + 1} ${direction} ${distanceKm}km`, rightX, y);
			});
		}
	}

	// Helper for rounded rectangle
	function roundRect(
		ctx: CanvasRenderingContext2D,
		x: number,
		y: number,
		w: number,
		h: number,
		r: number
	) {
		ctx.beginPath();
		ctx.moveTo(x + r, y);
		ctx.lineTo(x + w - r, y);
		ctx.quadraticCurveTo(x + w, y, x + w, y + r);
		ctx.lineTo(x + w, y + h - r);
		ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
		ctx.lineTo(x + r, y + h);
		ctx.quadraticCurveTo(x, y + h, x, y + h - r);
		ctx.lineTo(x, y + r);
		ctx.quadraticCurveTo(x, y, x + r, y);
		ctx.closePath();
	}

	$effect(() => {
		busProgress;
		routeData;
		highlightedEvent;
		render();
	});

	onMount(() => {
		ctx = canvas.getContext('2d');
		if (!ctx) return;

		updateSize();

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
