/**
 * Coordinate projection for GPS bus arrival detection system
 *
 * Uses flat-earth approximation with equirectangular projection.
 * Constants must match Rust implementation in shared/src/lib.rs
 */

// Earth's radius in centimeters
export const EARTH_R_CM = 637_100_000;

// Fixed origin longitude in degrees (120.0°E)
export const FIXED_ORIGIN_LON_DEG = 120.0;

// Fixed origin latitude in degrees (20.0°N)
export const FIXED_ORIGIN_LAT_DEG = 20.0;

// Fixed origin Y coordinate in centimeters (R_CM * (20.0 * PI / 180.0))
export const FIXED_ORIGIN_Y_CM = 222_389_853;

// Average latitude for projection (Taiwan: 25.0°N)
export const PROJECTION_LAT_AVG = 25.0;

/**
 * Convert degrees to radians
 */
function toRadians(degrees: number): number {
	return (degrees * Math.PI) / 180;
}

/**
 * Project GPS coordinates to local flat-earth coordinates (cm)
 *
 * This matches the Rust implementation in preprocessor/src/coord.rs
 *
 * @param lat - Latitude in degrees
 * @param lon - Longitude in degrees
 * @returns [x_cm, y_cm] coordinates relative to fixed origin
 */
export function projectLatLonToCm(lat: number, lon: number): [number, number] {
	const avg_lat_rad = toRadians(PROJECTION_LAT_AVG);
	const lon_rad = toRadians(lon);
	const lat_rad = toRadians(lat);

	// Equirectangular projection: x = R × cos(lat_avg) × Δlon
	const x_cm = Math.round(EARTH_R_CM * Math.cos(avg_lat_rad) * (lon_rad - toRadians(FIXED_ORIGIN_LON_DEG)));

	// y = R × Δlat (relative to fixed origin)
	const y_cm = Math.round(EARTH_R_CM * lat_rad - FIXED_ORIGIN_Y_CM);

	return [x_cm, y_cm];
}

/**
 * Convert local flat-earth coordinates (cm) back to GPS coordinates
 *
 * Inverse of projectLatLonToCm
 *
 * @param x_cm - X coordinate in cm
 * @param y_cm - Y coordinate in cm
 * @returns [lat, lon] in degrees
 */
export function projectCmToLatLon(x_cm: number, y_cm: number): [number, number] {
	const avg_lat_rad = toRadians(PROJECTION_LAT_AVG);

	// Inverse formulas
	const lon_rad = toRadians(FIXED_ORIGIN_LON_DEG) + x_cm / (EARTH_R_CM * Math.cos(avg_lat_rad));
	const lat_rad = (y_cm + FIXED_ORIGIN_Y_CM) / EARTH_R_CM;

	const lon = (lon_rad * 180) / Math.PI;
	const lat = (lat_rad * 180) / Math.PI;

	return [lat, lon];
}
