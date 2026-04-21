<script lang="ts">
	import type { StopTraceState } from '$lib/types';

	interface Props {
		stopState: StopTraceState;
		v_cms: number;
	}

	let { stopState, v_cms }: Props = $props();

	// Bayesian parameters from arrival_detector/src/probability.rs
	const SIGMA_D = 2750;
	const SIGMA_P = 2000;
	const V_STOP = 200;
	const K_SPEED = 0.01;
	const T_REF = 10;

	const weights = { p1: 13, p2: 6, p3: 10, p4: 3 };
	const weightSum = 32;

	// Mathematical PDF functions for curves
	const pdfs = {
		p1: (x: number) => Math.exp(-0.5 * Math.pow(x / SIGMA_D, 2)),
		p2: (x: number) => 1.0 / (1.0 + Math.exp(K_SPEED * (x - V_STOP))),
		p3: (x: number) => Math.exp(-0.5 * Math.pow(x / SIGMA_P, 2)),
		p4: (x: number) => Math.min(1.0, x / T_REF)
	};

	// X-axis ranges for visualization
	const ranges = {
		p1: { min: 0, max: 8000, label: 'Distance (cm)' },
		p2: { min: 0, max: 1200, label: 'Speed (cm/s)' },
		p3: { min: 0, max: 6000, label: 'Progress Diff (cm)' },
		p4: { min: 0, max: 15, label: 'Dwell (s)' }
	};

	function generatePath(key: keyof typeof pdfs, width: number, height: number) {
		const range = ranges[key];
		const pdf = pdfs[key];
		const points: string[] = [];
		const steps = 50;

		for (let i = 0; i <= steps; i++) {
			const xVal = range.min + (i / steps) * (range.max - range.min);
			const yVal = pdf(xVal);
			const x = (i / steps) * width;
			const y = height - yVal * height;
			points.push(`${x},${y}`);
		}

		return `M ${points.join(' L ')}`;
	}

	function getMarkerPos(key: keyof typeof pdfs, val: number, width: number) {
		const range = ranges[key];
		const clamped = Math.max(range.min, Math.min(range.max, val));
		return ((clamped - range.min) / (range.max - range.min)) * width;
	}

	function calculateContribution(value: number, weight: number): number {
		return Math.round((value * weight) / weightSum);
	}
</script>

<div class="scope-container">
	<div class="scope-header">
		<h3>Bayesian Probability Lab</h3>
		<div class="final-prob">
			<span class="label">FINAL P</span>
			<span class="value {stopState.probability >= 191 ? 'active' : ''}">{stopState.probability}</span>
			<span class="total">/255</span>
		</div>
	</div>

	<div class="scopes-grid">
		<!-- P1: Distance -->
		<div class="scope-card">
			<div class="scope-info">
				<span class="scope-name">p₁: GPS Dist</span>
				<span class="scope-weight">W: 13/32</span>
				<span class="scope-val">{stopState.features.p1}</span>
			</div>
			<div class="svg-wrapper">
				<svg viewBox="0 0 200 60" preserveAspectRatio="none">
					<path d={generatePath('p1', 200, 60)} class="pdf-curve" />
					<line 
						x1={getMarkerPos('p1', Math.abs(stopState.gps_distance_cm), 200)} 
						y1="0" 
						x2={getMarkerPos('p1', Math.abs(stopState.gps_distance_cm), 200)} 
						y2="60" 
						class="marker-line" 
					/>
				</svg>
				<div class="axis-labels">
					<span>0</span>
					<span>{Math.abs(stopState.gps_distance_cm)} cm</span>
					<span>80m</span>
				</div>
			</div>
			<div class="contribution">+{calculateContribution(stopState.features.p1, 13)} pts</div>
		</div>

		<!-- P2: Speed -->
		<div class="scope-card">
			<div class="scope-info">
				<span class="scope-name">p₂: Speed</span>
				<span class="scope-weight">W: 6/32</span>
				<span class="scope-val">{stopState.features.p2}</span>
			</div>
			<div class="svg-wrapper">
				<svg viewBox="0 0 200 60" preserveAspectRatio="none">
					<path d={generatePath('p2', 200, 60)} class="pdf-curve" />
					<line 
						x1={getMarkerPos('p2', v_cms, 200)} 
						y1="0" 
						x2={getMarkerPos('p2', v_cms, 200)} 
						y2="60" 
						class="marker-line" 
					/>
				</svg>
				<div class="axis-labels">
					<span>0</span>
					<span>{v_cms} cm/s</span>
					<span>12m/s</span>
				</div>
			</div>
			<div class="contribution">+{calculateContribution(stopState.features.p2, 6)} pts</div>
		</div>

		<!-- P3: Progress -->
		<div class="scope-card">
			<div class="scope-info">
				<span class="scope-name">p₃: Prog Dist</span>
				<span class="scope-weight">W: 10/32</span>
				<span class="scope-val">{stopState.features.p3}</span>
			</div>
			<div class="svg-wrapper">
				<svg viewBox="0 0 200 60" preserveAspectRatio="none">
					<path d={generatePath('p3', 200, 60)} class="pdf-curve" />
					<line 
						x1={getMarkerPos('p3', Math.abs(stopState.progress_distance_cm), 200)} 
						y1="0" 
						x2={getMarkerPos('p3', Math.abs(stopState.progress_distance_cm), 200)} 
						y2="60" 
						class="marker-line" 
					/>
				</svg>
				<div class="axis-labels">
					<span>0</span>
					<span>{Math.abs(stopState.progress_distance_cm)} cm</span>
					<span>60m</span>
				</div>
			</div>
			<div class="contribution">+{calculateContribution(stopState.features.p3, 10)} pts</div>
		</div>

		<!-- P4: Dwell -->
		<div class="scope-card">
			<div class="scope-info">
				<span class="scope-name">p₄: Dwell</span>
				<span class="scope-weight">W: 3/32</span>
				<span class="scope-val">{stopState.features.p4}</span>
			</div>
			<div class="svg-wrapper">
				<svg viewBox="0 0 200 60" preserveAspectRatio="none">
					<path d={generatePath('p4', 200, 60)} class="pdf-curve" />
					<line 
						x1={getMarkerPos('p4', stopState.dwell_time_s, 200)} 
						y1="0" 
						x2={getMarkerPos('p4', stopState.dwell_time_s, 200)} 
						y2="60" 
						class="marker-line" 
					/>
				</svg>
				<div class="axis-labels">
					<span>0</span>
					<span>{stopState.dwell_time_s} s</span>
					<span>15s</span>
				</div>
			</div>
			<div class="contribution">+{calculateContribution(stopState.features.p4, 3)} pts</div>
		</div>
	</div>

	<div class="formula">
		Σ = (13p₁ + 6p₂ + 10p₃ + 3p₄) / 32
	</div>
</div>

<style>
	.scope-container {
		background-color: #1e1e1e;
		color: #e0e0e0;
		padding: 1rem;
		border-radius: 0.5rem;
		border: 1px solid #333;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
	}

	.scope-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
		border-bottom: 1px solid #333;
		padding-bottom: 0.5rem;
	}

	.scope-header h3 {
		font-size: 0.875rem;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: #888;
		margin: 0;
	}

	.final-prob {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
	}

	.final-prob .label {
		font-size: 0.75rem;
		color: #888;
	}

	.final-prob .value {
		font-size: 1.5rem;
		font-weight: bold;
		color: #ef4444;
	}

	.final-prob .value.active {
		color: #22c55e;
		text-shadow: 0 0 10px rgba(34, 197, 94, 0.5);
	}

	.final-prob .total {
		font-size: 0.875rem;
		color: #555;
	}

	.scopes-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 1rem;
	}

	.scope-card {
		background-color: #252525;
		padding: 0.75rem;
		border-radius: 0.25rem;
		border: 1px solid #333;
	}

	.scope-info {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.5rem;
		font-size: 0.75rem;
	}

	.scope-name {
		color: #3b82f6;
		font-weight: bold;
	}

	.scope-weight {
		color: #666;
	}

	.scope-val {
		background-color: #000;
		padding: 2px 6px;
		border-radius: 2px;
		color: #00ff00;
	}

	.svg-wrapper {
		height: 60px;
		background-color: #000;
		border: 1px solid #333;
		position: relative;
		margin-bottom: 0.25rem;
	}

	svg {
		width: 100%;
		height: 100%;
	}

	.pdf-curve {
		fill: none;
		stroke: #3b82f6;
		stroke-width: 2;
		opacity: 0.8;
	}

	.marker-line {
		stroke: #ff00ff;
		stroke-width: 2;
		stroke-dasharray: 2, 2;
	}

	.axis-labels {
		display: flex;
		justify-content: space-between;
		font-size: 0.625rem;
		color: #555;
		padding: 0 2px;
	}

	.contribution {
		text-align: right;
		font-size: 0.75rem;
		color: #888;
		margin-top: 0.25rem;
	}

	.formula {
		margin-top: 1rem;
		text-align: center;
		font-size: 0.75rem;
		color: #555;
		background-color: #000;
		padding: 0.5rem;
		border-radius: 0.25rem;
	}
</style>
