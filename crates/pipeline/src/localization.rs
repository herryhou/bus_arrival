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
    pub accuracy_cm: Option<i32>,
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

    /// Builder method for accuracy_cm
    pub fn with_accuracy_cm(mut self, accuracy_cm: Option<i32>) -> Self {
        self.accuracy_cm = accuracy_cm;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics_builder() {
        let diag = GpsDiagnostics::new()
            .with_segment_idx(Some(5))
            .with_heading_met(true)
            .with_divergence_cm(100)
            .with_hdop(Some(1.5))
            .with_accuracy_cm(Some(150))
            .with_num_sats(Some(12))
            .with_fix_type(Some("3D".to_string()))
            .with_variance_cm2(50);

        assert_eq!(diag.segment_idx, Some(5));
        assert_eq!(diag.heading_met, true);
        assert_eq!(diag.divergence_cm, 100);
        assert_eq!(diag.hdop, Some(1.5));
        assert_eq!(diag.accuracy_cm, Some(150));
        assert_eq!(diag.num_sats, Some(12));
        assert_eq!(diag.fix_type, Some("3D".to_string()));
        assert_eq!(diag.variance_cm2, 50);
    }

    #[test]
    fn test_diagnostics_default() {
        let diag = GpsDiagnostics::new();

        assert_eq!(diag.segment_idx, None);
        assert_eq!(diag.heading_met, false);
        assert_eq!(diag.divergence_cm, 0);
        assert_eq!(diag.hdop, None);
        assert_eq!(diag.accuracy_cm, None);
        assert_eq!(diag.num_sats, None);
        assert_eq!(diag.fix_type, None);
        assert_eq!(diag.variance_cm2, 0);
    }
}
