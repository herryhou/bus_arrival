<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { TraceData, StopTraceState } from '$lib/types';
	import { Chart, type ChartConfiguration, type ChartType } from 'chart.js/auto';

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
	let speedChart: Chart | null = null;
	let probabilityChart: Chart | null = null;
	let distanceChart: Chart | null = null;

	// FSM state colors
	const stateColors: Record<string, string> = {
		Approaching: '#3b82f6', // blue
		Arriving: '#f59e0b', // amber
		AtStop: '#22c55e', // green
		Departed: '#6b7280' // gray
	};

	// Compute filtered data and arrays
	function getFilteredData() {
		return selectedStop !== null
			? traceData.filter((record) => record.active_stops.includes(selectedStop!))
			: traceData;
	}

	function getTimeLabels(data: typeof traceData) {
		return data.map((r) => new Date(r.time * 1000).toLocaleTimeString());
	}

	function getSpeedData(data: typeof traceData) {
		return data.map((r) => r.v_cms);
	}

	function getProbabilityData(data: typeof traceData) {
		return selectedStop !== null
			? data.map((r) => {
					const state = r.stop_states.find((s) => s.stop_idx === selectedStop);
					return state?.probability ?? 0;
				})
			: [];
	}

	function getDistanceData(data: typeof traceData) {
		return selectedStop !== null
			? data.map((r) => {
					const state = r.stop_states.find((s) => s.stop_idx === selectedStop);
					return state?.distance_cm ?? 0;
				})
			: [];
	}

	onMount(() => {
		const filteredData = getFilteredData();
		const timeLabels = getTimeLabels(filteredData);
		const speedData = getSpeedData(filteredData);

		// Speed chart
		const speedConfig: ChartConfiguration = {
			type: 'line',
			data: {
				labels: timeLabels,
				datasets: [
					{
						label: 'Speed (cm/s)',
						data: speedData,
						borderColor: '#3b82f6',
						backgroundColor: 'rgba(59, 130, 246, 0.1)',
						tension: 0.1,
						fill: true
					}
				]
			},
			options: {
				responsive: true,
				maintainAspectRatio: false,
				plugins: {
					legend: {
						display: true
					},
					tooltip: {
						intersect: false,
						mode: 'index'
					}
				},
				scales: {
					x: {
						display: true,
						title: {
							display: true,
							text: 'Time'
						}
					},
					y: {
						display: true,
						title: {
							display: true,
							text: 'Speed (cm/s)'
						}
					}
				},
				onClick: (event, elements) => {
					if (elements.length > 0) {
						const index = elements[0].index;
						onTimeChange(filteredData[index].time);
					}
				}
			}
		};

		speedChart = new Chart(document.createElement('canvas'), speedConfig);
		chartContainer.querySelector('.speed-chart-container')?.appendChild(speedChart.canvas);

		// Probability chart (only when stop is selected)
		if (selectedStop !== null) {
			const probabilityData = getProbabilityData(filteredData);
			const probConfig: ChartConfiguration = {
				type: 'line',
				data: {
					labels: timeLabels,
					datasets: [
						{
							label: 'Arrival Probability',
							data: probabilityData,
							borderColor: '#22c55e',
							backgroundColor: 'rgba(34, 197, 94, 0.1)',
							tension: 0.1,
							fill: true
						}
					]
				},
				options: {
					responsive: true,
					maintainAspectRatio: false,
					plugins: {
						legend: {
							display: true
						},
						tooltip: {
							intersect: false,
							mode: 'index'
						}
					},
					scales: {
						x: {
							display: true,
							title: {
								display: true,
								text: 'Time'
							}
						},
						y: {
							display: true,
							min: 0,
							max: 255,
							title: {
								display: true,
								text: 'Probability (0-255)'
							}
						}
					},
					onClick: (event, elements) => {
						if (elements.length > 0) {
							const index = elements[0].index;
							onTimeChange(filteredData[index].time);
						}
					}
				}
			};

			probabilityChart = new Chart(document.createElement('canvas'), probConfig);
			chartContainer.querySelector('.probability-chart-container')?.appendChild(probabilityChart.canvas);
		}

		// Distance chart (only when stop is selected)
		if (selectedStop !== null) {
			const distanceData = getDistanceData(filteredData);
			const distConfig: ChartConfiguration = {
				type: 'line',
				data: {
					labels: timeLabels,
					datasets: [
						{
							label: 'Distance to Stop (cm)',
							data: distanceData,
							borderColor: '#ef4444',
							backgroundColor: 'rgba(239, 68, 68, 0.1)',
							tension: 0.1,
							fill: true
						}
					]
				},
				options: {
					responsive: true,
					maintainAspectRatio: false,
					plugins: {
						legend: {
							display: true
						},
						tooltip: {
							intersect: false,
							mode: 'index'
						}
					},
					scales: {
						x: {
							display: true,
							title: {
								display: true,
								text: 'Time'
							}
						},
						y: {
							display: true,
							title: {
								display: true,
								text: 'Distance (cm)'
							}
						}
					},
					onClick: (event, elements) => {
						if (elements.length > 0) {
							const index = elements[0].index;
							onTimeChange(filteredData[index].time);
						}
					}
				}
			};

			distanceChart = new Chart(document.createElement('canvas'), distConfig);
			chartContainer.querySelector('.distance-chart-container')?.appendChild(distanceChart.canvas);
		}
	});

	onDestroy(() => {
		speedChart?.destroy();
		probabilityChart?.destroy();
		distanceChart?.destroy();
	});
</script>

<div class="charts-container" bind:this={chartContainer}>
	<!-- Speed chart (always shown) -->
	<div class="chart-wrapper">
		<h3 class="chart-title">Speed Over Time</h3>
		<div class="speed-chart-container chart-canvas"></div>
	</div>

	{#if selectedStop !== null}
		<!-- Probability chart (when stop selected) -->
		<div class="chart-wrapper">
			<h3 class="chart-title">Arrival Probability (Stop {selectedStop})</h3>
			<div class="probability-chart-container chart-canvas"></div>
		</div>

		<!-- Distance chart (when stop selected) -->
		<div class="chart-wrapper">
			<h3 class="chart-title">Distance to Stop (Stop {selectedStop})</h3>
			<div class="distance-chart-container chart-canvas"></div>
		</div>
	{/if}
</div>

<style>
	.charts-container {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 1rem;
		background-color: #f9fafb;
		border-radius: 0.5rem;
	}

	.chart-wrapper {
		background-color: white;
		border-radius: 0.375rem;
		padding: 1rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.chart-title {
		font-size: 0.875rem;
		font-weight: 600;
		color: #374151;
		margin-bottom: 0.5rem;
	}

	.chart-canvas {
		height: 200px;
		width: 100%;
	}
</style>
