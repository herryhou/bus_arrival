//! Localization state management

/// GPS diagnostics information
///
/// Encapsulates diagnostic data from GPS processing.
/// Replaces the 7-parameter tuple approach.
#[derive(Debug, Clone, Default)]
pub struct GpsDiagnostics {
    pub segment_idx: Option<u16>,
    pub heading_met: bool,
    pub divergence_cm: i32,
    pub hdop: Option<f32>,
    pub num_sats: Option<u8>,
    pub fix_type: Option<String>,
    pub variance_cm2: i32,
}

impl GpsDiagnostics {
    /// Create new diagnostics with minimal fields
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method for segment_idx
    pub fn with_segment_idx(mut self, idx: Option<u16>) -> Self {
        self.segment_idx = idx;
        self
    }

    /// Builder method for heading_met
    pub fn with_heading_met(mut self, met: bool) -> Self {
        self.heading_met = met;
        self
    }

    /// Builder method for divergence_cm
    pub fn with_divergence_cm(mut self, div: i32) -> Self {
        self.divergence_cm = div;
        self
    }

    /// Builder method for hdop
    pub fn with_hdop(mut self, hdop: Option<f32>) -> Self {
        self.hdop = hdop;
        self
    }

    /// Builder method for num_sats
    pub fn with_num_sats(mut self, sats: Option<u8>) -> Self {
        self.num_sats = sats;
        self
    }

    /// Builder method for fix_type
    pub fn with_fix_type(mut self, fix: Option<String>) -> Self {
        self.fix_type = fix;
        self
    }

    /// Builder method for variance_cm2
    pub fn with_variance_cm2(mut self, var: i32) -> Self {
        self.variance_cm2 = var;
        self
    }
}
