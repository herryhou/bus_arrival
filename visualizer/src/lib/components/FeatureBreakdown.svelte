<script lang="ts">
	import type { TraceData, StopTraceState } from '$lib/types';

	interface Props {
		traceData: TraceData;
		selectedStop?: number | null;
		currentTime: number;
	}

	let { traceData, selectedStop = null, currentTime }: Props = $props();

	// Find the trace record closest to current time
	function getCurrentRecord() {
		return traceData.find((r) => r.time === currentTime)
			?? traceData.reduce((closest, record) => {
				return Math.abs(record.time - currentTime) < Math.abs(closest.time - currentTime)
					? record
					: closest;
			}, traceData[0]);
	}

	// Get the stop state for the selected stop at current time
	function getStopState() {
		const currentRecord = getCurrentRecord();
		return selectedStop !== null && currentRecord
			? currentRecord.stop_states.find((s) => s.stop_idx === selectedStop)
			: null;
	}

	// Feature descriptions
	const features = [
		{
			key: 'p1',
			name: 'Distance Likelihood',
			description: 'Gaussian distribution: higher when close to stop (σ=2750cm)',
			color: 'bg-blue-500'
		},
		{
			key: 'p2',
			name: 'Speed Likelihood',
			description: 'Logistic function: higher when slow (v_stop=200cm/s)',
			color: 'bg-amber-500'
		},
		{
			key: 'p3',
			name: 'Progress Likelihood',
			description: 'Gaussian distribution: progress difference (σ=2000cm)',
			color: 'bg-green-500'
		},
		{
			key: 'p4',
			name: 'Dwell Time Likelihood',
			description: 'Linear function: increases with dwell time (max at 10s)',
			color: 'bg-purple-500'
		}
	];

	// Weight coefficients from Rust implementation
	const weights = { p1: 13, p2: 6, p3: 10, p4: 3 };
	const weightSum = 32;

	// Calculate weighted contribution
	function calculateContribution(value: number, weight: number): number {
		return Math.round((value * weight) / weightSum);
	}
</script>

<div class="breakdown-container">
	{#if getStopState()}
		{@const stopState = getStopState()}
		<!-- Final Probability -->
		<div class="probability-summary">
			<div class="summary-header">
				<h3>Final Probability</h3>
				<div class="probability-badge">
					<span class="probability-value">{stopState.probability}</span>
					<span class="probability-max">/ 255</span>
				</div>
			</div>
			<div class="probability-bar">
				<div
					class="probability-fill"
					style="width: {(stopState.probability / 255) * 100}%; background-color: {stopState.probability > 191
						? '#22c55e'
						: stopState.probability > 128
						? '#f59e0b'
						: '#ef4444'};"
				></div>
			</div>
			<div class="threshold-line" style="left: {(191 / 255) * 100}%"></div>
			<div class="threshold-label">Threshold: 191</div>
		</div>

		<!-- Feature Breakdown -->
		<div class="features-list">
			<h3 class="list-title">Feature Breakdown</h3>

			{#each features as feature}
				{@const value = stopState.features[feature.key as keyof typeof stopState.features]}
				{@const weight = weights[feature.key as keyof typeof weights]}
				{@const contribution = calculateContribution(value, weight)}

				<div class="feature-item">
					<div class="feature-header">
						<div class="feature-info">
							<div class="feature-dot {feature.color}"></div>
							<div>
								<div class="feature-name">{feature.name}</div>
								<div class="feature-desc">{feature.description}</div>
							</div>
						</div>
						<div class="feature-value">{value}</div>
					</div>

					<!-- Value bar -->
					<div class="value-bar-container">
						<div class="value-bar {feature.color}" style="width: {(value / 255) * 100}%"></div>
					</div>

					<!-- Weight and contribution -->
					<div class="feature-meta">
						<span class="weight-label">Weight: {weight}/32</span>
						<span class="contribution-label">Contribution: +{contribution}</span>
					</div>
				</div>
			{/each}
		</div>

		<!-- Formula explanation -->
		<div class="formula-card">
			<h3 class="card-title">Bayesian Formula</h3>
			<div class="formula">
				P = (13×p₁ + 6×p₂ + 10×p₃ + 3×p₄) / 32
			</div>
			<div class="formula-explanation">
				<p>Final probability is a weighted sum of individual feature scores.</p>
				<p>Arrival threshold: 191/255 ≈ 75%</p>
			</div>
		</div>
	{:else}
		<div class="empty-state">
			<p>Select a stop to see feature breakdown</p>
		</div>
	{/if}
</div>

<style>
	.breakdown-container {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 1rem;
		background-color: #f9fafb;
		border-radius: 0.5rem;
		height: 100%;
		overflow-y: auto;
	}

	.probability-summary {
		background-color: white;
		border-radius: 0.375rem;
		padding: 1rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
		position: relative;
	}

	.summary-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.75rem;
	}

	.summary-header h3 {
		font-size: 0.875rem;
		font-weight: 600;
		color: #374151;
		margin: 0;
	}

	.probability-badge {
		display: flex;
		align-items: baseline;
		gap: 0.25rem;
	}

	.probability-value {
		font-size: 1.5rem;
		font-weight: 700;
		color: #111827;
	}

	.probability-max {
		font-size: 0.875rem;
		color: #6b7280;
	}

	.probability-bar {
		position: relative;
		height: 2rem;
		background-color: #e5e7eb;
		border-radius: 0.25rem;
		overflow: hidden;
	}

	.probability-fill {
		height: 100%;
		transition: width 0.3s ease, background-color 0.3s ease;
	}

	.threshold-line {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 2px;
		background-color: #111827;
		z-index: 1;
	}

	.threshold-label {
		font-size: 0.625rem;
		color: #6b7280;
		margin-top: 0.25rem;
	}

	.features-list {
		background-color: white;
		border-radius: 0.375rem;
		padding: 1rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.list-title {
		font-size: 0.875rem;
		font-weight: 600;
		color: #374151;
		margin-bottom: 1rem;
	}

	.feature-item {
		padding: 0.75rem 0;
		border-bottom: 1px solid #e5e7eb;
	}

	.feature-item:last-child {
		border-bottom: none;
	}

	.feature-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.5rem;
	}

	.feature-info {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.feature-dot {
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.feature-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: #111827;
	}

	.feature-desc {
		font-size: 0.75rem;
		color: #6b7280;
	}

	.feature-value {
		font-size: 1rem;
		font-weight: 700;
		color: #374151;
	}

	.value-bar-container {
		height: 0.5rem;
		background-color: #e5e7eb;
		border-radius: 0.25rem;
		overflow: hidden;
		margin-bottom: 0.25rem;
	}

	.value-bar {
		height: 100%;
		transition: width 0.3s ease;
	}

	.feature-meta {
		display: flex;
		justify-content: space-between;
		font-size: 0.75rem;
		color: #6b7280;
	}

	.weight-label,
	.contribution-label {
		background-color: #f3f4f6;
		padding: 0.125rem 0.375rem;
		border-radius: 0.25rem;
	}

	.formula-card {
		background-color: white;
		border-radius: 0.375rem;
		padding: 1rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.card-title {
		font-size: 0.875rem;
		font-weight: 600;
		color: #374151;
		margin-bottom: 0.75rem;
	}

	.formula {
		font-family: 'Courier New', monospace;
		font-size: 0.875rem;
		color: #111827;
		background-color: #f3f4f6;
		padding: 0.75rem;
		border-radius: 0.25rem;
		text-align: center;
		margin-bottom: 0.75rem;
	}

	.formula-explanation {
		font-size: 0.75rem;
		color: #6b7280;
	}

	.formula-explanation p {
		margin: 0.25rem 0;
	}

	.empty-state {
		background-color: white;
		border-radius: 0.375rem;
		padding: 2rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
		text-align: center;
	}

	.empty-state p {
		color: #9ca3af;
		font-size: 0.875rem;
	}

	.bg-blue-500 {
		background-color: #3b82f6;
	}

	.bg-amber-500 {
		background-color: #f59e0b;
	}

	.bg-green-500 {
		background-color: #22c55e;
	}

	.bg-purple-500 {
		background-color: #a855f7;
	}
</style>
