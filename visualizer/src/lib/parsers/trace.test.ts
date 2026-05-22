import { describe, expect, it } from 'vitest';
import { filterTraceByTime, getTraceTimeRange, parseTraceJsonl } from './trace';

const baseTraceV2 = {
	gps: {
		time_ms: 80_001_000,
		lat: 25,
		lon: 121,
		heading_cdeg: 1800,
		hdop: 1.5,
		accuracy_cm: 1500,
		num_sats: 12,
		fix_type: '3d'
	},
	kalman: {
		s_cm: 100,
		v_cms: 10,
		variance_cm2: 2500,
		divergence_cm: 100
	},
	map_matching: {
		segment_idx: 42,
		heading_constraint_met: true
	},
	detection: {
		status: 'valid' as const,
		off_route: false,
		gps_jump: false,
		recovery_idx: null,
		off_route_last_s_cm: null
	},
	corridor: {
		active_stops: [],
		corridor_start_cm: null,
		corridor_end_cm: null,
		next_stop: null
	},
	stop_states: []
};

const baseTraceLegacy = {
	time_ms: 80_001_000,
	lat: 25,
	lon: 121,
	s_cm: 100,
	v_cms: 10,
	heading_cdeg: 1800,
	active_stops: [],
	stop_states: [],
	gps_jump: false,
	recovery_idx: null,
	segment_idx: 42,
	heading_constraint_met: true,
	divergence_cm: 100,
	hdop: 1.5,
	variance_cm2: 2500,
	corridor_start_cm: null,
	corridor_end_cm: null,
	next_stop: null,
	off_route: false,
	status: 'valid' as const
};

describe('trace parser (v2 and legacy)', () => {
	describe('v2 grouped format', () => {
		it('parses grouped v2 schema', () => {
			const trace = parseTraceJsonl(JSON.stringify(baseTraceV2));

			expect(trace[0].gps.time_ms).toBe(80_001_000);
			expect(trace[0].gps.lat).toBe(25);
			expect(trace[0].gps.lon).toBe(121);
			expect(trace[0].kalman.s_cm).toBe(100);
			expect(trace[0].kalman.v_cms).toBe(10);
			expect(trace[0].map_matching.segment_idx).toBe(42);
			expect(trace[0].detection.status).toBe('valid');
			expect(trace[0].corridor.active_stops).toEqual([]);
		});

		it('uses milliseconds for ranges and filters', () => {
			const trace = parseTraceJsonl(
				[
					JSON.stringify({ ...baseTraceV2, gps: { ...baseTraceV2.gps, time_ms: 1_000 } }),
					JSON.stringify({ ...baseTraceV2, gps: { ...baseTraceV2.gps, time_ms: 2_000 } })
				].join('\n')
			);

			expect(getTraceTimeRange(trace)).toEqual([1_000, 2_000]);
			expect(filterTraceByTime(trace, 1_500, 2_000).map((record) => record.gps.time_ms)).toEqual([
				2_000
			]);
		});

		it('extracts unique stop indices from corridor.active_stops', () => {
			const trace = parseTraceJsonl(
				[
					JSON.stringify({ ...baseTraceV2, corridor: { ...baseTraceV2.corridor, active_stops: [5, 6] } }),
					JSON.stringify({ ...baseTraceV2, corridor: { ...baseTraceV2.corridor, active_stops: [6, 7] } })
				].join('\n')
			);

			const stops = new Set(
				trace.flatMap((record) => record.corridor.active_stops)
			);
			expect(Array.from(stops).sort((a, b) => a - b)).toEqual([5, 6, 7]);
		});
	});

	describe('legacy flat format', () => {
		it('converts legacy flat format to v2', () => {
			const trace = parseTraceJsonl(JSON.stringify(baseTraceLegacy));

			expect(trace[0].gps.time_ms).toBe(80_001_000);
			expect(trace[0].gps.lat).toBe(25);
			expect(trace[0].gps.lon).toBe(121);
			expect(trace[0].kalman.s_cm).toBe(100);
			expect(trace[0].kalman.v_cms).toBe(10);
			expect(trace[0].map_matching.segment_idx).toBe(42);
			expect(trace[0].detection.status).toBe('valid');
			expect(trace[0].corridor.active_stops).toEqual([]);
		});

		it('handles legacy time (seconds) field', () => {
			const trace = parseTraceJsonl(JSON.stringify({ ...baseTraceLegacy, time: 80_001 }));

			expect(trace[0].gps.time_ms).toBe(80_001_000);
		});

		it('uses milliseconds for ranges and filters with legacy format', () => {
			const trace = parseTraceJsonl(
				[
					JSON.stringify({ ...baseTraceLegacy, time_ms: 1_000 }),
					JSON.stringify({ ...baseTraceLegacy, time_ms: 2_000 })
				].join('\n')
			);

			expect(getTraceTimeRange(trace)).toEqual([1_000, 2_000]);
			expect(filterTraceByTime(trace, 1_500, 2_000).map((record) => record.gps.time_ms)).toEqual([
				2_000
			]);
		});
	});
});
