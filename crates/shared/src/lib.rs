//! Shared types for GPS bus arrival detection system.
//!
//! All physical quantities use semantic integer types to prevent unit confusion
//! and enable zero-cost runtime behavior on no_std targets.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// Import Ord trait for .max() method (needed for no_std)
use core::cmp::Ord;
use core::option::Option::{self, None, Some};

/// Earth's radius in centimeters
pub const EARTH_R_CM: f64 = 637_100_000.0;

/// Fixed origin longitude in degrees (120.0°E)
pub const FIXED_ORIGIN_LON_DEG: f64 = 120.0;

/// Fixed origin latitude in degrees (20.0°N)
pub const FIXED_ORIGIN_LAT_DEG: f64 = 20.0;

/// Fixed origin Y coordinate in centimeters
pub const FIXED_ORIGIN_Y_CM: i64 = 222389853; // R_CM * (20.0 * PI / 180.0)

pub mod binfile;

/// Distance in centimeters.
/// Range: ±21,474,836 cm ≈ ±214 km — sufficient for bus routes.
pub type DistCm = i32;

/// Speed in centimeters per second.
/// Range: 0..21,474,836 cm/s ≈ 0..214 km/h — covers bus speeds.
pub type SpeedCms = i32;

/// Heading in hundredths of a degree.
/// Range: -18000..18000 = -180°..+180°
pub type HeadCdeg = i16;

/// Geographic coordinate in hundredths of a degree.
/// Range: -18000..18000 = -180°..+180°
/// Used for latitude and longitude (NOT heading/direction).
pub type GeoCdeg = i16;

/// Probability scaled 0..255 (u8 = probability × 255).
/// Precision: 1/256 ≈ 0.004 — sufficient for arrival decisions.
pub type Prob8 = u8;

/// Squared distance (cm²) for intermediate calculations.
/// Prevents overflow in dot products: (2×10⁶)² ≈ 4×10¹² < i64::MAX.
pub type Dist2 = i64;

/// Route node with precomputed segment coefficients for runtime GPS matching.
///
/// Field ordering: i32 fields grouped first to satisfy 4-byte alignment
/// without compiler-inserted padding on ARM Cortex-M33.
/// Total size = 24 bytes (no padding required).
///
/// # Layout (v8.7 - 24 bytes)
/// ```text
/// offset  0: x_cm         i32   4 bytes
/// offset  4: y_cm         i32   4 bytes
/// offset  8: cum_dist_cm  i32   4 bytes
/// offset 12: seg_len_mm   i32   4 bytes  (|P\[i+1\]-P\[i\]|, mm)
/// offset 16: dx_cm        i16   2 bytes  (segment vector x)
/// offset 18: dy_cm        i16   2 bytes  (segment vector y)
/// offset 20: heading_cdeg i16   2 bytes
/// offset 22: _pad         i16   2 bytes  (alignment padding to 4-byte boundary)
/// total: 24 bytes
/// ```
///
/// # Changes from v8.5
/// - Removed `len2_cm2` (i64) - computed at runtime as (seg_len_mm / 10)^2
/// - Changed `seg_len_cm` (i32) to `seg_len_mm` (i32) for 10x precision
/// - Changed `dx_cm`, `dy_cm` from i32 to i16 (max segment length 100m = 10,000 cm fits in i16)
/// - Reordered fields for optimal packing (24 bytes total)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RouteNode {
    // ── i32 fields first (4-byte aligned) ──────────────────────────
    /// X coordinate (relative to grid origin) in cm
    pub x_cm: DistCm,
    /// Y coordinate (relative to grid origin) in cm
    pub y_cm: DistCm,
    /// Cumulative distance from route start in cm
    pub cum_dist_cm: DistCm,
    /// Segment length: |P[i+1] - P[i]| in millimeters
    pub seg_len_mm: i32,

    // ── i16 fields (2-byte aligned) ────────────────────────────────
    /// Segment vector X: x\[i+1\] - x\[i\] in cm
    pub dx_cm: i16,
    /// Segment vector Y: y\[i+1\] - y\[i\] in cm
    pub dy_cm: i16,
    /// Segment heading in 0.01° (e.g., 9000 = 90°)
    pub heading_cdeg: HeadCdeg,
    /// Padding to align struct size to 4-byte boundary
    pub _pad: i16,
}


/// Bus stop with precomputed corridor boundaries.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stop {
    /// Position along route in cm
    pub progress_cm: DistCm,
    /// Corridor start: progress_cm - 8000 cm (80m before stop)
    pub corridor_start_cm: DistCm,
    /// Corridor end: progress_cm + 4000 cm (40m after stop)
    pub corridor_end_cm: DistCm,
}

/// Grid origin for spatial indexing.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GridOrigin {
    /// Fixed origin X coordinate (cm)
    pub x0_cm: DistCm,
    /// Fixed origin Y coordinate (cm)
    pub y0_cm: DistCm,
}

/// Parsed GPS data from NMEA sentences.
/// All fields use integer types for no-FPU compatibility.
#[derive(Debug, Clone)]
pub struct GpsPoint {
    pub timestamp: u64, // seconds since epoch
    pub lat_cdeg: GeoCdeg, // Latitude in 0.01° units (e.g., 235000 = 23.5000°N)
    pub lon_cdeg: GeoCdeg, // Longitude in 0.01° units (e.g., 1205000 = 120.5000°E)
    pub heading_cdeg: HeadCdeg, // Heading in 0.01° units
    pub speed_cms: SpeedCms, // Speed in cm/s
    pub hdop_x10: u16, // HDOP * 10 (e.g., 15 = 1.5)
    pub has_fix: bool,
}

impl GpsPoint {
    pub fn new() -> Self {
        GpsPoint {
            timestamp: 0,
            lat_cdeg: 0,
            lon_cdeg: 0,
            heading_cdeg: 0,
            speed_cms: 0,
            hdop_x10: 0,
            has_fix: false,
        }
    }
}

/// 1D Kalman filter state for route progress estimation.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct KalmanState {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub last_seg_idx: usize,
}

impl KalmanState {
    pub fn new() -> Self {
        KalmanState { s_cm: 0, v_cms: 0, last_seg_idx: 0 }
    }

    /// Cold start initialization from first valid GPS fix.
    /// Should be paired with 3-second warm-up period (see tech report Section 19.5).
    pub fn init(z_cm: DistCm, v_gps_cms: SpeedCms, seg_idx: usize) -> Self {
        KalmanState {
            s_cm: z_cm,
            v_cms: v_gps_cms,
            last_seg_idx: seg_idx,
        }
    }

    pub fn update(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms) {
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (51 * (z_cm - s_pred)) / 256;
        self.v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0);
    }

    pub fn update_adaptive(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms, hdop_x10: u16) {
        let ks = Self::ks_from_hdop(hdop_x10);
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (ks * (z_cm - s_pred)) / 256;
        self.v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0);
    }

    fn ks_from_hdop(hdop_x10: u16) -> i32 {
        match hdop_x10 {
            0..=20 => 77,
            21..=30 => 51,
            31..=50 => 26,
            _ => 13,
        }
    }
}

/// Spatial grid for O(k) map matching (used by preprocessor to build).
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,
    pub grid_size_cm: DistCm,
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: DistCm,
    pub y0_cm: DistCm,
}

#[cfg(feature = "std")]
impl SpatialGrid {
    /// Create an empty spatial grid
    pub fn empty() -> Self {
        SpatialGrid {
            cells: vec![vec![]],
            grid_size_cm: 10000,
            cols: 0,
            rows: 0,
            x0_cm: 0,
            y0_cm: 0,
        }
    }

    /// Query grid for candidate segments around a point
    pub fn query(&self, x_cm: DistCm, y_cm: DistCm) -> Vec<usize> {
        if self.cols == 0 || self.rows == 0 {
            return Vec::new();
        }

        let gx = ((x_cm - self.x0_cm) / self.grid_size_cm) as usize;
        let gy = ((y_cm - self.y0_cm) / self.grid_size_cm) as usize;

        let mut candidates = Vec::new();

        // 3×3 neighborhood
        for dy in 0..=2 {
            for dx in 0..=2 {
                let ny = (gy as i32 + dy as i32 - 1) as usize;
                let nx = (gx as i32 + dx as i32 - 1) as usize;

                if ny < self.rows as usize && nx < self.cols as usize {
                    let idx = ny * (self.cols as usize) + nx;
                    candidates.extend_from_slice(&self.cells[idx]);
                }
            }
        }

        candidates
    }
}

/// Dead-reckoning state for GPS outage compensation.
#[derive(Debug, Clone)]
pub struct DrState {
    pub last_gps_time: Option<u64>,
    pub last_valid_s: DistCm,
    pub filtered_v: SpeedCms,
}

impl DrState {
    pub fn new() -> Self {
        DrState {
            last_gps_time: None,
            last_valid_s: 0,
            filtered_v: 0,
        }
    }
}

/// Stop state machine states
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FsmState {
    /// Bus is idle (before entering corridor)
    Idle,
    /// Bus is approaching stop (in corridor, not yet close)
    Approaching,
    /// Bus is in arrival zone (close to stop)
    Arriving,
    /// Bus has arrived (confirmed stop)
    AtStop,
    /// Bus has departed (moved past stop)
    Departed,
    /// Trip completed (past last stop, terminal state)
    TripComplete,
}

/// Arrival event emitted when bus reaches a stop
#[cfg_attr(feature = "serde", derive(Debug, Clone, serde::Serialize))]
pub struct ArrivalEvent {
    /// GPS update timestamp (seconds since epoch)
    pub time: u64,
    /// Stop index that was arrived at
    pub stop_idx: u8,
    /// Route progress at arrival (cm)
    pub s_cm: DistCm,
    /// Speed at arrival (cm/s)
    pub v_cms: SpeedCms,
    /// Arrival probability that triggered
    pub probability: Prob8,
}

/// Departure event emitted when bus leaves a stop
#[cfg_attr(feature = "serde", derive(Debug, Clone, serde::Serialize))]
pub struct DepartureEvent {
    /// GPS update timestamp (seconds since epoch)
    pub time: u64,
    /// Stop index that was departed from
    pub stop_idx: u8,
    /// Route progress at departure (cm)
    pub s_cm: DistCm,
    /// Speed at departure (cm/s)
    pub v_cms: SpeedCms,
}

// Compile-time assertion — v8.7: 24 bytes (no padding)
const _: () = assert!(core::mem::size_of::<RouteNode>() == 24);
const _: () = assert!(core::mem::size_of::<Stop>() == 12);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kalman_v_cms_non_negative_constraint() {
        // v8.5: GPS noise can produce negative velocity, which causes
        // predict step to move backward, triggering chain rejections.
        // The .max(0) constraint prevents this.

        let mut state = KalmanState::init(10000, 100, 5);

        // Simulate GPS noise: negative speed measurement
        state.update(10100, -500);  // GPS reports -500 cm/s (noise)

        // v_cms should be clamped to 0, not negative
        assert_eq!(state.v_cms, 0, "v_cms should be clamped to 0, got {}", state.v_cms);
    }

    #[test]
    fn test_kalman_update_adaptive_v_cms_non_negative() {
        let mut state = KalmanState::init(10000, 100, 5);

        // Test with various HDOP levels, all should respect non-negative constraint
        for hdop in [10, 25, 40, 100] {
            let v_before = state.v_cms;
            state.update_adaptive(10100, -1000, hdop);
            assert!(state.v_cms >= 0, "v_cms should be non-negative for HDOP {}", hdop);
        }
    }

    #[test]
    fn test_kalman_init_cold_start() {
        // v8.5: New init() method for cold start from first valid GPS fix
        let state = KalmanState::init(50000, 200, 10);

        assert_eq!(state.s_cm, 50000, "s_cm should be initialized");
        assert_eq!(state.v_cms, 200, "v_cms should be initialized");
        assert_eq!(state.last_seg_idx, 10, "last_seg_idx should be initialized");
    }

    #[test]
    fn test_kalman_normal_update_preserves_positive_velocity() {
        let mut state = KalmanState::init(10000, 500, 5);

        // Normal GPS update with positive speed
        state.update(10500, 600);

        // v_cms should remain positive
        assert!(state.v_cms > 0, "v_cms should remain positive with normal GPS data");
    }

    #[test]
    fn test_kalman_severe_noise_clamps_to_zero() {
        let mut state = KalmanState::init(10000, 100, 5);

        // Extreme GPS noise: very negative speed
        state.update(10100, -10000);

        // v_cms should be clamped to 0
        assert_eq!(state.v_cms, 0, "v_cms should be clamped to 0 with extreme noise");
    }
}
