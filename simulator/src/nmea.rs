//! NMEA sentence parser

use shared::{GpsPoint, SpeedCms, HeadCdeg};

/// NMEA parser state (accumulates data across sentences)
pub struct NmeaState {
    point: GpsPoint,
}

impl NmeaState {
    pub fn new() -> Self {
        NmeaState {
            point: GpsPoint::new(),
        }
    }

    /// Parse NMEA sentence, returns Some(GpsPoint) when complete
    pub fn parse_sentence(&mut self, sentence: &str) -> Option<GpsPoint> {
        if !verify_checksum(sentence) {
            return None;
        }

        let parts: Vec<&str> = sentence.split(',').collect();

        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "$GPRMC" => self.parse_rmc(&parts),
            "$GNGSA" => self.parse_gsa(&parts),
            "$GPGGA" => self.parse_gga(&parts),
            _ => None,
        }
    }

    fn parse_rmc(&mut self, parts: &[&str]) -> Option<GpsPoint> {
        // $GPRMC,123519,A,ddmm.mm,s,dddmm.mm,a,hhh.h,V,V,ddmmyy,,,A*hh
        if parts.len() < 12 {
            return None;
        }

        // Status 'V' = Warning, 'A' = Valid
        let status = parts[2];
        if status != "A" {
            return None;
        }

        // Parse position
        let lat = parse_lat(parts[3], parts[4])?;
        let lon = parse_lon(parts[5], parts[6])?;
        let speed_knots: f64 = parts[7].parse().unwrap_or(0.0);
        let heading_deg: f64 = parts[8].parse().unwrap_or(0.0);

        self.point.lat = lat;
        self.point.lon = lon;
        self.point.speed_cms = knots_to_cms(speed_knots);
        self.point.heading_cdeg = (heading_deg * 100.0).round() as HeadCdeg;
        self.point.has_fix = true;

        None // Not complete yet (need HDOP)
    }

    fn parse_gsa(&mut self, parts: &[&str]) -> Option<GpsPoint> {
        // $GNGSA,A,3,04,05,...,xx,xx,xx*hh
        // Last three fields are PDOP, HDOP, VDOP
        if parts.len() < 17 {
            return None;
        }

        // HDOP is second-to-last field (index -2)
        let hdop_idx = parts.len() - 2;
        let hdop: f64 = parts[hdop_idx].parse().unwrap_or(99.0);
        self.point.hdop_x10 = (hdop * 10.0).round() as u16;

        // Return complete point
        Some(std::mem::replace(&mut self.point, GpsPoint::new()))
    }

    fn parse_gga(&mut self, parts: &[&str]) -> Option<GpsPoint> {
        // $GPGGA,123519,v,ddmm.mm,s,dddmm.mm,a,xx,yy,z.z,h.h,M*hh
        if parts.len() < 9 {
            return None;
        }

        // Quality indicator
        if parts[6] != "1" && parts[6] != "2" {
            return None;
        }

        let lat = parse_lat(parts[2], parts[3])?;
        let lon = parse_lon(parts[4], parts[5])?;
        let hdop: f64 = parts[8].parse().unwrap_or(99.0);

        self.point.lat = lat;
        self.point.lon = lon;
        self.point.hdop_x10 = (hdop * 10.0).round() as u16;
        self.point.has_fix = true;

        // GGA alone is enough to complete the point
        Some(std::mem::replace(&mut self.point, GpsPoint::new()))
    }
}

/// Verify NMEA checksum
fn verify_checksum(sentence: &str) -> bool {
    if let Some(star_pos) = sentence.find('*') {
        let data = &sentence[1..star_pos];
        let checksum_str = &sentence[star_pos + 1..star_pos + 3];
        if let Ok(checksum) = u8::from_str_radix(checksum_str, 16) {
            let calculated = data.bytes().fold(0u8, |acc, b| acc ^ b);
            calculated == checksum
        } else {
            false
        }
    } else {
        false
    }
}

/// Parse latitude from NMEA format (ddmm.mmmm)
fn parse_lat(deg_min: &str, ns: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    Some(if ns == "N" { degrees } else { -degrees })
}

/// Parse longitude from NMEA format (dddmm.mmmm)
fn parse_lon(deg_min: &str, ew: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    Some(if ew == "E" { degrees } else { -degrees })
}

/// Convert knots to cm/s: 1 knot = 51.44 cm/s
fn knots_to_cms(knots: f64) -> SpeedCms {
    (knots * 51.44).round() as SpeedCms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_checksum_valid() {
        assert!(verify_checksum("$GPRMC,123519,V,0000.0000,N,00000.0000,E,000.0,000.0,030311,,,A*6A"));
    }

    #[test]
    fn verify_checksum_invalid() {
        assert!(!verify_checksum("$GPRMC,123519,V,0000.0000,N,00000.0000,E,000.0,000.0,030311,,,A*00"));
    }

    #[test]
    fn parse_lat_north() {
        assert_eq!(parse_lat("2502.5434", "N").unwrap(), 25.04239);
    }

    #[test]
    fn parse_lat_south() {
        assert_eq!(parse_lat("2502.5434", "S").unwrap(), -25.04239);
    }

    #[test]
    fn parse_lon_east() {
        assert_eq!(parse_lon("12117.1898", "E").unwrap(), 121.28649666666667);
    }

    #[test]
    fn parse_lon_west() {
        assert_eq!(parse_lon("12117.1898", "W").unwrap(), -121.28649666666667);
    }

    #[test]
    fn knots_to_cms_conversion() {
        assert_eq!(knots_to_cms(10.0), 514); // ~5.1 m/s = 514 cm/s
        assert_eq!(knots_to_cms(0.0), 0);
        assert_eq!(knots_to_cms(1.0), 51); // 1 knot ≈ 51.44 cm/s
    }

    #[test]
    fn parse_rmc_valid() {
        let mut state = NmeaState::new();
        let result = state.parse_sentence("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        assert!(result.is_none()); // RMC alone doesn't complete the point
        assert!(state.point.has_fix);
        assert!((state.point.lat - 25.004303333333333).abs() < 1e-10);
        assert!((state.point.lon - 121.28649666666667).abs() < 1e-10);
        assert_eq!(state.point.speed_cms, 432); // 8.4 knots * 51.44
        assert_eq!(state.point.heading_cdeg, 8050); // 80.5° * 100
    }

    #[test]
    fn parse_rmc_invalid_status() {
        let mut state = NmeaState::new();
        let result = state.parse_sentence("$GPRMC,221320,V,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*39");
        assert!(result.is_none());
        assert!(!state.point.has_fix);
    }

    #[test]
    fn parse_gga_valid() {
        let mut state = NmeaState::new();
        let result = state.parse_sentence("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
        assert!(result.is_none()); // GGA alone doesn't complete the point
        assert!(state.point.has_fix);
        assert!((state.point.lat - 25.004303333333333).abs() < 1e-10);
        assert!((state.point.lon - 121.28649666666667).abs() < 1e-10);
    }

    #[test]
    fn parse_gga_invalid_fix() {
        let mut state = NmeaState::new();
        let result = state.parse_sentence("$GPGGA,221320,2500.2582,N,12117.1898,E,0,08,3.5,10.0,M,0.0,M,,*4A");
        assert!(result.is_none());
        assert!(!state.point.has_fix);
    }

    #[test]
    fn parse_gsa_completes_point() {
        let mut state = NmeaState::new();
        // First, populate with RMC
        state.parse_sentence("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        // Then GSA should complete the point
        // $GNGSA,A,3,04,05,09,12,14,15,16,21,22,24,25,26,1.5,1.2,3.0
        // HDOP is at index 15 (value 1.2)
        let result = state.parse_sentence("$GNGSA,A,3,04,05,09,12,14,15,16,21,22,24,25,26,1.5,1.2,3.0*23");
        assert!(result.is_some());
        let point = result.unwrap();
        assert!(point.has_fix);
        assert!((point.lat - 25.004303333333333).abs() < 1e-10);
        assert!((point.lon - 121.28649666666667).abs() < 1e-10);
        assert_eq!(point.hdop_x10, 12); // 1.2 * 10
    }

    #[test]
    fn parse_sentence_unknown_type() {
        let mut state = NmeaState::new();
        let result = state.parse_sentence("$GPVTG,360.0,T,348.8,M,0.0,N,0.0,K*4D");
        assert!(result.is_none());
    }

    #[test]
    fn parse_sentence_invalid_checksum() {
        let mut state = NmeaState::new();
        let result = state.parse_sentence("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,350.5,141123,,*00");
        assert!(result.is_none());
    }
}
