//! GPS record types for pipeline processing

use shared::{DistCm, SpeedCms, HeadCdeg};

/// GPS record emitted by localization phase
#[derive(Debug, Clone)]
pub struct GpsRecord {
    /// GPS timestamp (seconds since epoch)
    pub time: u64,
    /// Latitude
    pub lat: f64,
    /// Longitude
    pub lon: f64,
    /// Route progress (cm)
    pub s_cm: DistCm,
    /// Velocity (cm/s)
    pub v_cms: SpeedCms,
    /// Heading (hundredths of degrees, -18000 to 18000)
    pub heading_cdeg: Option<HeadCdeg>,
    /// Processing status (for trace output)
    pub status: &'static str,

    // === Diagnostic fields for trace output ===
    /// Which route segment we're matched to (None if off-route)
    pub segment_idx: Option<u16>,
    /// Did the heading constraint pass? (±90° rule)
    pub heading_constraint_met: bool,
    /// Raw GPS projection - Kalman filtered position (cm)
    pub divergence_cm: i32,
    /// GPS quality: HDOP (None if not available)
    pub hdop: Option<f32>,
    /// GPS quality: number of satellites (None if not available)
    pub num_sats: Option<u8>,
    /// GPS quality: fix type - "none", "2d", "3d" (None if not available)
    pub fix_type: Option<String>,
    /// Kalman filter variance (cm²)
    pub variance_cm2: i32,
}

impl GpsRecord {
    /// Create a new GPS record
    pub fn new(
        time: u64,
        lat: f64,
        lon: f64,
        s_cm: DistCm,
        v_cms: SpeedCms,
        heading_cdeg: Option<HeadCdeg>,
        status: &'static str,
    ) -> Self {
        Self {
            time,
            lat,
            lon,
            s_cm,
            v_cms,
            heading_cdeg,
            status,
            // Diagnostic fields default to None/0
            segment_idx: None,
            heading_constraint_met: false,
            divergence_cm: 0,
            hdop: None,
            num_sats: None,
            fix_type: None,
            variance_cm2: 0,
        }
    }

    /// Builder method to set diagnostic fields
    pub fn with_diagnostics(
        mut self,
        segment_idx: Option<u16>,
        heading_constraint_met: bool,
        divergence_cm: i32,
        hdop: Option<f32>,
        num_sats: Option<u8>,
        fix_type: Option<String>,
        variance_cm2: i32,
    ) -> Self {
        self.segment_idx = segment_idx;
        self.heading_constraint_met = heading_constraint_met;
        self.divergence_cm = divergence_cm;
        self.hdop = hdop;
        self.num_sats = num_sats;
        self.fix_type = fix_type;
        self.variance_cm2 = variance_cm2;
        self
    }
}
