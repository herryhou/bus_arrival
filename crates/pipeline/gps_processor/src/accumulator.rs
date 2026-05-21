//! NMEA sentence accumulator for timestamp-driven GPS processing
//!
//! Accumulates NMEA sentences across UART reads, emitting exactly once
//! per GPS timestamp change. GPS timestamp is the only authority.

use shared::{FixQuality, GpsPoint, HeadCdeg, SpeedCms};

#[allow(dead_code)]
const MAX_NMEA_FIELDS: usize = 20;

/// Accumulates NMEA sentences into a single GPS fix per timestamp.
///
/// # Emission Semantics
///
/// Emits the **previous** snapshot when a **new** timestamp arrives:
/// ```text
/// RMC(t=01) + GGA(t=01) → accumulate
/// RMC(t=02) arrives → should_emit() true → emit (t=01) → reset
/// ```
///
/// # First Fix Edge Case
///
/// Does NOT emit on first timestamp. Only emits after seeing second
/// timestamp (requires `last_emitted_timestamp.is_some()`).
pub struct FixAccumulator {
    timestamp: Option<u64>,
    lat: Option<f64>,
    lon: Option<f64>,
    speed: Option<SpeedCms>,
    heading: Option<HeadCdeg>,
    hdop: Option<u16>,
    has_fix: bool,
    last_emitted_timestamp: Option<u64>,
}

impl Default for FixAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl FixAccumulator {
    pub fn new() -> Self {
        FixAccumulator {
            timestamp: None,
            lat: None,
            lon: None,
            speed: None,
            heading: None,
            hdop: None,
            has_fix: false,
            last_emitted_timestamp: None,
        }
    }

    /// Parse and update accumulator with an NMEA sentence.
    /// Returns true if the sentence was valid and contributed data.
    pub fn update(&mut self, sentence: &str) -> bool {
        if !verify_checksum(sentence) {
            return false;
        }

        #[cfg(feature = "std")]
        let parts: Vec<&str> = sentence.split(',').collect();
        #[cfg(not(feature = "std"))]
        let parts: heapless::Vec<&str, MAX_NMEA_FIELDS> = sentence.split(',').collect();

        if parts.is_empty() {
            return false;
        }

        let parts_slice: &[&str] = &parts;
        match parts_slice.first() {
            Some(&"$GPRMC") | Some(&"$GNRMC") => self.update_rmc(parts_slice),
            Some(&"$GNGSA") | Some(&"$GPGSA") => self.update_gsa(parts_slice),
            Some(&"$GPGGA") | Some(&"$GNGGA") => self.update_gga(parts_slice),
            _ => false,
        }
    }

    /// Check if timestamp changed.
    /// Returns true if we should emit the previous snapshot.
    ///
    /// Updates `last_emitted_timestamp` when returning true.
    pub fn should_emit(&mut self) -> bool {
        let ts = match self.timestamp {
            Some(t) => t,
            None => return false,
        };
        match self.last_emitted_timestamp {
            None => {
                // First timestamp - don't emit, but remember we've seen it
                self.last_emitted_timestamp = Some(ts);
                false
            }
            Some(last_ts) if last_ts != ts => {
                // New timestamp - emit the previous snapshot
                self.last_emitted_timestamp = Some(ts);
                true
            }
            _ => false,
        }
    }

    /// Mark the current timestamp as emitted.
    /// Call this after building the GPS data.
    pub fn mark_emitted(&mut self) {
        // This is called after build(), so we don't need to do anything
        // The last_emitted_timestamp is already updated by should_emit()
    }

    /// Build a GpsPoint if we have minimum required data.
    /// Returns None if insufficient data for a valid fix.
    pub fn build(&self) -> Option<(GpsPoint, FixQuality)> {
        if !self.has_fix {
            return None;
        }
        let lat = self.lat?;
        let lon = self.lon?;
        let timestamp = self.timestamp?;

        let has_motion = self.speed.is_some() || self.heading.is_some();
        let has_quality = self.hdop.is_some();

        let quality = match (has_motion, has_quality) {
            (true, true) => FixQuality::Full,
            (false, true) => FixQuality::PositionOnly,
            (true, false) => FixQuality::MotionOnly,
            (false, false) => FixQuality::PositionOnly,
        };

        Some((
            GpsPoint {
                timestamp,
                lat,
                lon,
                speed_cms: self.speed,
                heading_cdeg: self.heading,
                accuracy_cm: None,
                hdop_x10: self.hdop,
                has_fix: true,
            },
            quality,
        ))
    }

    /// Reset accumulator state for next second.
    /// Call this AFTER emitting a fix.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn update_rmc(&mut self, parts: &[&str]) -> bool {
        if parts.len() < 12 {
            return false;
        }

        if parts[2] != "A" {
            return false;
        }

        let lat = match parse_lat(parts[3], parts[4]) {
            Some(l) => l,
            None => return false,
        };
        let lon = match parse_lon(parts[5], parts[6]) {
            Some(l) => l,
            None => return false,
        };
        let speed_knots: f64 = parts[7].parse().unwrap_or(0.0);
        let heading_deg: f64 = parts[8].parse().unwrap_or(0.0);

        let heading_cdeg = (heading_deg * 100.0) as i32;
        let heading_cdeg = if heading_cdeg > 18000 {
            heading_cdeg - 36000
        } else {
            heading_cdeg
        };

        if parts[1].len() >= 6 {
            let hh: u64 = parts[1][0..2].parse().unwrap_or(0);
            let mm: u64 = parts[1][2..4].parse().unwrap_or(0);
            let ss: u64 = parts[1][4..6].parse().unwrap_or(0);
            self.timestamp = Some((hh * 3600 + mm * 60 + ss) * 1000);
        }

        self.lat = Some(lat);
        self.lon = Some(lon);
        self.speed = Some(knots_to_cms(speed_knots));
        self.heading = Some(heading_cdeg as HeadCdeg);
        self.has_fix = true;
        true
    }

    fn update_gsa(&mut self, parts: &[&str]) -> bool {
        if parts.len() < 17 {
            return false;
        }

        let hdop_idx = parts.len() - 2;
        let hdop: f64 = parts[hdop_idx].parse().unwrap_or(99.0);
        self.hdop = Some((hdop * 10.0) as u16);
        true
    }

    fn update_gga(&mut self, parts: &[&str]) -> bool {
        if parts.len() < 9 {
            return false;
        }

        if parts[6] != "1" && parts[6] != "2" {
            return false;
        }

        let lat = match parse_lat(parts[2], parts[3]) {
            Some(l) => l,
            None => return false,
        };
        let lon = match parse_lon(parts[4], parts[5]) {
            Some(l) => l,
            None => return false,
        };
        let hdop: f64 = parts[8].parse().unwrap_or(99.0);

        if parts[1].len() >= 6 {
            let hh: u64 = parts[1][0..2].parse().unwrap_or(0);
            let mm: u64 = parts[1][2..4].parse().unwrap_or(0);
            let ss: u64 = parts[1][4..6].parse().unwrap_or(0);
            self.timestamp = Some((hh * 3600 + mm * 60 + ss) * 1000);
        }

        self.lat = Some(lat);
        self.lon = Some(lon);
        self.hdop = Some((hdop * 10.0) as u16);
        self.has_fix = true;
        true
    }
}

fn verify_checksum(sentence: &str) -> bool {
    if let Some(star_pos) = sentence.find('*') {
        let data = &sentence[1..star_pos];
        let checksum_str = &sentence[star_pos + 1..star_pos + 3];
        if let Ok(checksum) = u8::from_str_radix(checksum_str, 16) {
            let calculated = data.bytes().fold(0u8, |acc, b| acc ^ b);
            return calculated == checksum;
        }
    }
    false
}

pub fn parse_lat(deg_min: &str, ns: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    #[cfg(feature = "std")]
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    #[cfg(not(feature = "std"))]
    let degrees = libm::truncf(dm as f32 / 100.0) as f64 + ((dm % 100.0) as f32 / 60.0) as f64;
    Some(if ns == "N" { degrees } else { -degrees })
}

pub fn parse_lon(deg_min: &str, ew: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    #[cfg(feature = "std")]
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    #[cfg(not(feature = "std"))]
    let degrees = libm::truncf(dm as f32 / 100.0) as f64 + ((dm % 100.0) as f32 / 60.0) as f64;
    Some(if ew == "E" { degrees } else { -degrees })
}

pub fn knots_to_cms(knots: f64) -> SpeedCms {
    (knots * 51.44) as SpeedCms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_accumulator_is_empty() {
        let mut acc = FixAccumulator::new();
        assert!(!acc.should_emit());
        assert!(acc.build().is_none());
    }

    #[test]
    fn test_rmc_only_creates_motion_only_fix() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");

        assert!(!acc.should_emit());
        assert!(acc.build().is_some());

        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::MotionOnly);
        assert!(gps.speed_cms.is_some());
        assert!(gps.heading_cdeg.is_some());
        assert!(gps.hdop_x10.is_none());
    }

    #[test]
    fn test_gga_only_creates_position_only_fix() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

        assert!(!acc.should_emit());
        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::PositionOnly);
        assert!(gps.speed_cms.is_none());
        assert!(gps.heading_cdeg.is_none());
        assert!(gps.hdop_x10.is_some());
    }

    #[test]
    fn test_rmc_then_gga_creates_full_fix() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::Full);
        assert!(gps.speed_cms.is_some());
        assert!(gps.heading_cdeg.is_some());
        assert!(gps.hdop_x10.is_some());
    }

    #[test]
    fn test_emit_on_second_timestamp() {
        let mut acc = FixAccumulator::new();

        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        assert!(!acc.should_emit());

        acc.update("$GPRMC,221321,A,2500.2583,N,12117.1899,E,8.5,81.5,141123,,*2F");
        assert!(acc.should_emit());

        let (gps, _) = acc.build().unwrap();
        assert_eq!(gps.timestamp, (22 * 3600 + 13 * 60 + 21) * 1000);  // NEW timestamp, not old
    }

    #[test]
    fn test_out_of_order_sentences() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");

        let (_gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::Full);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        acc.reset();

        assert!(acc.build().is_none());
        assert!(!acc.should_emit());
    }

    #[test]
    fn test_invalid_checksum_rejected() {
        let mut acc = FixAccumulator::new();
        let result = acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*00");
        assert!(!result);
        assert!(acc.build().is_none());
    }
}
