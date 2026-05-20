import { describe, expect, it } from 'vitest';
import { filterTraceByTime, getTraceTimeRange, parseTraceJsonl } from './trace';

const baseTrace = {
	lat: 25,
	lon: 121,
	s_cm: 100,
	v_cms: 10,
	active_stops: [],
	stop_states: [],
	gps_jump: false,
	recovery_idx: null,
	heading_constraint_met: true,
	divergence_cm: 0,
	variance_cm2: 0
};

describe('trace parser timestamp semantics', () => {
	it('keeps canonical time_ms as milliseconds', () => {
		const trace = parseTraceJsonl(JSON.stringify({ ...baseTrace, time_ms: 80_001_000 }));

		expect(trace[0].time_ms).toBe(80_001_000);
		expect('time' in trace[0]).toBe(false);
	});

	it('converts legacy time seconds to time_ms', () => {
		const trace = parseTraceJsonl(JSON.stringify({ ...baseTrace, time: 80_001 }));

		expect(trace[0].time_ms).toBe(80_001_000);
		expect('time' in trace[0]).toBe(false);
	});

	it('uses milliseconds for ranges and filters', () => {
		const trace = parseTraceJsonl(
			[
				JSON.stringify({ ...baseTrace, time_ms: 1_000 }),
				JSON.stringify({ ...baseTrace, time_ms: 2_000 })
			].join('\n')
		);

		expect(getTraceTimeRange(trace)).toEqual([1_000, 2_000]);
		expect(filterTraceByTime(trace, 1_500, 2_000).map((record) => record.time_ms)).toEqual([
			2_000
		]);
	});
});
