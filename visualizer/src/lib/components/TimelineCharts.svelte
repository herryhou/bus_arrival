<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { TraceData } from '$lib/types';
	import { Chart, type ChartConfiguration } from 'chart.js/auto';

	interface Props {
		traceData: TraceData;
		selectedStop?: number | null;
		currentTime: number;
		onTimeChange?: (time: number) => void;
	}

	let {
		traceData,
		selectedStop = null,
		currentTime,
		onTimeChange = () => {}
	}: Props = $props();

	let chartContainer: HTMLDivElement;
	let mainChart: Chart | null = null;

	function updateChart() {
		if (!mainChart) return;

		const data = selectedStop !== null
			? traceData.filter((record) => record.active_stops.includes(selectedStop!))
			: traceData;

		const timeLabels = data.map((r) => new Date(r.time * 1000).toLocaleTimeString([], { hour12: false }));
		
		mainChart.data.labels = timeLabels;
		mainChart.data.datasets[0].data = data.map(r => r.v_cms);
		
		if (selectedStop !== null) {
			mainChart.data.datasets[1].data = data.map(r => {
				const s = r.stop_states.find(ss => ss.stop_idx === selectedStop);
				return s ? s.probability : 0;
			});
			mainChart.data.datasets[1].hidden = false;
		} else {
			mainChart.data.datasets[1].hidden = true;
		}

		mainChart.update('none');
	}

	onMount(() => {
		const ctx = document.createElement('canvas');
		chartContainer.appendChild(ctx);

		const config: ChartConfiguration = {
			type: 'line',
			data: {
				labels: [],
				datasets: [
					{
						label: 'Speed (cm/s)',
						data: [],
						borderColor: '#3b82f6',
						backgroundColor: 'rgba(59, 130, 246, 0.1)',
						borderWidth: 1.5,
						pointRadius: 0,
						tension: 0.1,
						fill: true,
						yAxisID: 'y'
					},
					{
						label: 'Probability (0-255)',
						data: [],
						borderColor: '#22c55e',
						backgroundColor: 'rgba(34, 197, 94, 0.1)',
						borderWidth: 1.5,
						pointRadius: 0,
						tension: 0.1,
						fill: true,
						yAxisID: 'y1',
						hidden: true
					}
				]
			},
			options: {
				responsive: true,
				maintainAspectRatio: false,
				animation: false,
				interaction: {
					intersect: false,
					mode: 'index'
				},
				plugins: {
					legend: {
						display: true,
						labels: { color: '#888', font: { size: 10, family: 'JetBrains Mono' } }
					},
					tooltip: {
						backgroundColor: '#1a1a1a',
						titleColor: '#fff',
						bodyColor: '#ccc',
						borderColor: '#333',
						borderWidth: 1
					}
				},
				scales: {
					x: {
						ticks: { color: '#444', font: { size: 9 } },
						grid: { color: '#222' }
					},
					y: {
						type: 'linear',
						display: true,
						position: 'left',
						ticks: { color: '#3b82f6', font: { size: 9 } },
						grid: { color: '#222' },
						title: { display: true, text: 'Speed', color: '#3b82f6', font: { size: 10 } }
					},
					y1: {
						type: 'linear',
						display: true,
						position: 'right',
						min: 0,
						max: 255,
						ticks: { color: '#22c55e', font: { size: 9 } },
						grid: { drawOnChartArea: false },
						title: { display: true, text: 'Prob', color: '#22c55e', font: { size: 10 } }
					}
				},
				onClick: (event, elements) => {
					if (elements.length > 0) {
						const index = elements[0].index;
						const data = selectedStop !== null
							? traceData.filter((record) => record.active_stops.includes(selectedStop!))
							: traceData;
						onTimeChange(data[index].time);
					}
				}
			}
		};

		mainChart = new Chart(ctx, config);
		updateChart();
	});

	$effect(() => {
		updateChart();
	});

	onDestroy(() => {
		mainChart?.destroy();
	});
</script>

<div class="charts-container" bind:this={chartContainer}></div>

<style>
	.charts-container {
		width: 100%;
		height: 100%;
		background-color: #0a0a0a;
		padding: 0.5rem 1rem;
	}
</style>
