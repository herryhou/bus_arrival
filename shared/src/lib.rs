//! Shared types for GPS bus arrival detection system.
//!
//! All physical quantities use semantic integer types to prevent unit confusion
//! and enable zero-cost runtime behavior on no_std targets.

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

/// Probability scaled 0..255 (u8 = probability × 255).
/// Precision: 1/256 ≈ 0.004 — sufficient for arrival decisions.
pub type Prob8 = u8;

/// Squared distance (cm²) for intermediate calculations.
/// Prevents overflow in dot products: (2×10⁶)² ≈ 4×10¹² < i64::MAX.
pub type Dist2 = i64;

/// Route node with ALL precomputed segment coefficients.
///
/// Field ordering: i64 fields placed first to satisfy 8-byte alignment
/// without compiler-inserted padding on ARM Cortex-M33.
/// Total size = 52 bytes (verified at compile time).
///
/// # Layout
/// ```text
/// offset  0: len2_cm2     i64   8 bytes  (|P[i+1]-P[i]|², cm²)
/// offset  8: line_c       i64   8 bytes  (= -(A·x₀ + B·y₀))
/// offset 16: x_cm         i32   4 bytes
/// offset 20: y_cm         i32   4 bytes
/// offset 24: cum_dist_cm  i32   4 bytes
/// offset 28: dx_cm        i32   4 bytes  (segment vector x)
/// offset 32: dy_cm        i32   4 bytes  (segment vector y)
/// offset 36: seg_len_cm   i32   4 bytes  (offline sqrt, not used runtime)
/// offset 40: line_a       i32   4 bytes  (= -dy)
/// offset 44: line_b       i32   4 bytes  (= dx)
/// offset 48: heading_cdeg i16   2 bytes
/// offset 50: _pad         i16   2 bytes
/// total: 52 bytes (no padding gaps)
/// ```
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RouteNode {
    // ── i64 fields first ──────────────────────────────────────────
    /// Squared segment length: |P[i+1] - P[i]|² in cm²
    pub len2_cm2: Dist2,
    /// Line constant: -(line_a × x₀ + line_b × y₀)
    pub line_c: Dist2,

    // ── i32 fields ────────────────────────────────────────────────
    /// X coordinate (relative to grid origin) in cm
    pub x_cm: DistCm,
    /// Y coordinate (relative to grid origin) in cm
    pub y_cm: DistCm,
    /// Cumulative distance from route start in cm
    pub cum_dist_cm: DistCm,
    /// Segment vector X: x[i+1] - x[i] in cm
    pub dx_cm: DistCm,
    /// Segment vector Y: y[i+1] - y[i] in cm
    pub dy_cm: DistCm,
    /// Segment length in cm (sqrt computed offline only)
    pub seg_len_cm: DistCm,
    /// Line coefficient A: = -dy_cm (for distance calculation)
    pub line_a: DistCm,
    /// Line coefficient B: = dx_cm (for distance calculation)
    pub line_b: DistCm,

    // ── i16 fields ────────────────────────────────────────────────
    /// Segment heading in 0.01° (e.g., 9000 = 90°)
    pub heading_cdeg: HeadCdeg,
    /// Padding to align struct size
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
#[derive(Debug, Clone)]
pub struct GpsPoint {
    pub lat: f64,
    pub lon: f64,
    pub heading_cdeg: HeadCdeg,
    pub speed_cms: SpeedCms,
    pub hdop_x10: u16,
    pub has_fix: bool,
}

impl GpsPoint {
    pub fn new() -> Self {
        GpsPoint {
            lat: 0.0,
            lon: 0.0,
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
}

impl KalmanState {
    pub fn new() -> Self {
        KalmanState { s_cm: 0, v_cms: 0 }
    }

    pub fn update(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms) {
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (51 * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }

    pub fn update_adaptive(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms, hdop_x10: u16) {
        let ks = Self::ks_from_hdop(hdop_x10);
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (ks * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
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
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,
    pub grid_size_cm: DistCm,
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: DistCm,
    pub y0_cm: DistCm,
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

// Compile-time assertion — fails if field reordering changes size
const _: () = assert!(core::mem::size_of::<RouteNode>() == 52);
const _: () = assert!(core::mem::size_of::<Stop>() == 12);
