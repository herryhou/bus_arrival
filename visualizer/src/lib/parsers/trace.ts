/**
 * Trace JSONL parser
 *
 * Parses both legacy flat/semi-grouped and new v2 grouped trace formats
 * Format: one JSON object per line (JSONL)
 */

import type { TraceRecord, TraceData } from '$lib/types';

/**
 * Convert legacy flat/semi-grouped format to v2 grouped format
 */
function convertLegacyToV2(raw: any): TraceRecord {
	// Handle time_ms vs time (seconds)
	const timeMs = typeof raw.time_ms === 'number' ? raw.time_ms :
	               typeof raw.time === 'number' ? raw.time * 1000 :
	               null;
	if (timeMs === null) {
		throw new Error('missing or invalid time field');
	}

	return {
		gps: {
			time_ms: timeMs,
			lat: raw.lat,
			lon: raw.lon,
			heading_cdeg: raw.heading_cdeg,
			hdop: raw.hdop ?? null,
			accuracy_cm: raw.accuracy_cm ?? null,
			num_sats: raw.num_sats ?? null,
			fix_type: raw.fix_type ?? null
		},
		kalman: {
			s_cm: raw.s_cm,
			v_cms: raw.v_cms,
			variance_cm2: raw.variance_cm2 ?? 0,
			divergence_cm: raw.divergence_cm ?? 0
		},
		map_matching: {
			segment_idx: raw.segment_idx ?? null,
			heading_constraint_met: raw.heading_constraint_met ?? true
		},
		detection: {
			status: raw.status ?? 'valid',
			off_route: raw.off_route ?? false,
			gps_jump: raw.gps_jump ?? false,
			recovery_idx: raw.recovery_idx ?? null,
			off_route_last_s_cm: raw.off_route_last_s_cm ?? null
		},
		corridor: {
			active_stops: raw.active_stops ?? [],
			corridor_start_cm: raw.corridor_start_cm ?? null,
			corridor_end_cm: raw.corridor_end_cm ?? null,
			next_stop: raw.next_stop ?? null
		},
		stop_states: raw.stop_states ?? []
	};
}

/**
 * Parse a trace.jsonl file content (supports both legacy and v2 formats)
 *
 * @param content - Raw file content (one JSON per line)
 * @returns Array of trace records
 * @throws Error if JSON parsing fails
 */
export function parseTraceJsonl(content: string): TraceData {
	const lines = content.split('\n').filter((line) => line.trim() !== '');

	const records: TraceRecord[] = [];
	let isV2Format = false;

	// Detect format from first non-empty line
	for (const line of lines) {
		try {
			const raw = JSON.parse(line);
			if (raw.gps && typeof raw.gps === 'object' && raw.kalman) {
				isV2Format = true;
			}
			break;
		} catch {
			continue;
		}
	}

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i].trim();
		if (!line) continue;

		try {
			const raw = JSON.parse(line);

			if (isV2Format) {
				// V2 grouped format - validate and use directly
				if (!raw.gps || typeof raw.gps !== 'object') {
					throw new Error(`Line ${i + 1}: missing or invalid 'gps' group`);
				}
				if (!raw.kalman || typeof raw.kalman !== 'object') {
					throw new Error(`Line ${i + 1}: missing or invalid 'kalman' group`);
				}
				if (!raw.map_matching || typeof raw.map_matching !== 'object') {
					throw new Error(`Line ${i + 1}: missing or invalid 'map_matching' group`);
				}
				if (!raw.detection || typeof raw.detection !== 'object') {
					throw new Error(`Line ${i + 1}: missing or invalid 'detection' group`);
				}
				if (!raw.corridor || typeof raw.corridor !== 'object') {
					throw new Error(`Line ${i + 1}: missing or invalid 'corridor' group`);
				}
				if (!Array.isArray(raw.stop_states)) {
					throw new Error(`Line ${i + 1}: missing or invalid 'stop_states' field`);
				}

				records.push({
					gps: raw.gps,
					kalman: raw.kalman,
					map_matching: raw.map_matching,
					detection: raw.detection,
					corridor: raw.corridor,
					stop_states: raw.stop_states
				});
			} else {
				// Legacy format - convert to v2
				records.push(convertLegacyToV2(raw));
			}
		} catch (e) {
			if (e instanceof SyntaxError) {
				throw new Error(`Line ${i + 1}: invalid JSON - ${e.message}`);
			}
			throw e;
		}
	}

	console.log(`parseTraceJsonl: parsed ${records.length} records, isV2Format=${isV2Format}`);
	return records;
}

/**
 * Load and parse a trace.jsonl file from a URL or File
 *
 * @param file - File object or URL to fetch
 * @returns Promise resolving to parsed trace data
 */
export async function loadTraceFile(file: File | string): Promise<TraceData> {
	let content: string;

	if (typeof file === 'string') {
		// Fetch from URL
		const response = await fetch(file);
		if (!response.ok) {
			throw new Error(`Failed to load trace file: ${response.statusText}`);
		}
		content = await response.text();
	} else {
		// Read from File object
		content = await file.text();
	}

	return parseTraceJsonl(content);
}

/**
 * Get time range from trace data
 *
 * @param data - Trace data array
 * @returns [min_time, max_time] in milliseconds
 */
export function getTraceTimeRange(data: TraceData): [number, number] {
	if (data.length === 0) return [0, 0];

	let minTime = data[0].gps.time_ms;
	let maxTime = data[0].gps.time_ms;

	for (const record of data) {
		if (record.gps.time_ms < minTime) minTime = record.gps.time_ms;
		if (record.gps.time_ms > maxTime) maxTime = record.gps.time_ms;
	}

	return [minTime, maxTime];
}

/**
 * Filter trace records by time range
 *
 * @param data - Trace data array
 * @param startTime - Start time in milliseconds
 * @param endTime - End time in milliseconds
 * @returns Filtered trace data
 */
export function filterTraceByTime(data: TraceData, startTime: number, endTime: number): TraceData {
	return data.filter((record) => record.gps.time_ms >= startTime && record.gps.time_ms <= endTime);
}

/**
 * Get all unique stop indices from trace data
 *
 * @param data - Trace data array
 * @returns Sorted array of unique stop indices
 */
export function getUniqueStopIndices(data: TraceData): number[] {
	const stops = new Set<number>();

	for (const record of data) {
		for (const stopIdx of record.corridor.active_stops) {
			stops.add(stopIdx);
		}
	}

	return Array.from(stops).sort((a, b) => a - b);
}
