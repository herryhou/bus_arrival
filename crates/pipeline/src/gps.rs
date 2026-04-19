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
        }
    }
}
