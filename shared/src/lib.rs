// Semantic type aliases for GPS bus arrival detection system
//
// These type aliases provide domain-specific meaning to primitive types,
// making the code more self-documenting and preventing unit errors.

/// Distance in centimeters.
///
/// # Range
/// - i32::MIN to i32::MAX (-2,147,483,648 to 2,147,483,647 cm)
/// - Approximately ±21,474 km or ±13,345 miles
///
/// # Use Cases
/// - GPS coordinates distance calculations
/// - Distance from bus stop
/// - Movement thresholds
pub type DistCm = i32;

/// Speed in centimeters per second.
///
/// # Range
/// - 0 to i32::MAX (0 to 2,147,483,647 cm/s)
/// - 0 to approximately 21,474 km/h or 13,345 mph
///
/// # Use Cases
/// - Bus velocity from GPS
/// - Speed filtering (e.g., ignore buses > 5 km/h)
/// - Motion detection thresholds
pub type SpeedCms = i32;

/// Heading in centidegrees (0.01° units).
///
/// # Range
/// - i16::MIN to i16::MAX (-32,768 to 32,767 centidegrees)
/// - -327.68° to 327.67° (typically used as -180° to 180°)
///
/// # Notes
/// - 0° = North, 90° = East, 180° = South, -90° = West
/// - Centidegree precision provides ~1m accuracy at 10km distance
///
/// # Use Cases
/// - Bus travel direction
/// - Approach direction validation
/// - Heading-based filtering
pub type HeadCdeg = i16;

/// Probability in 8-bit fixed-point (0..255).
///
/// # Range
/// - 0 to 255 (u8)
/// - Interpreted as 0.0 to 1.0 when divided by 255
/// - Or as percentage 0% to 100% when divided by 2.55
///
/// # Notes
/// - Compact representation for probabilities
/// - Suitable for storage and transmission
/// - Convert to f64 with: `value as f64 / 255.0`
///
/// # Use Cases
/// - GPS confidence levels
/// - Detection probabilities
/// - Classification scores
pub type Prob8 = u8;

/// Squared distance in square centimeters.
///
/// # Range
/// - i64::MIN to i64::MAX (very large range)
/// - Can store squared distances up to ~4.6×10¹⁸ cm²
///
/// # Notes
/// - Used to avoid expensive sqrt() operations in comparisons
/// - Compare squared distances instead of actual distances
/// - Convert with: `sqrt(d2) as DistCm`
///
/// # Use Cases
/// - Distance threshold comparisons (e.g., d2 < 10000² for <1km)
/// - Finding minimum/maximum distances
/// - Filtering by proximity without sqrt
pub type Dist2 = i64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_sizes() {
        // Verify that type aliases have the expected memory sizes
        assert_eq!(std::mem::size_of::<DistCm>(), 4, "DistCm should be 4 bytes (i32)");
        assert_eq!(std::mem::size_of::<SpeedCms>(), 4, "SpeedCms should be 4 bytes (i32)");
        assert_eq!(std::mem::size_of::<HeadCdeg>(), 2, "HeadCdeg should be 2 bytes (i16)");
        assert_eq!(std::mem::size_of::<Prob8>(), 1, "Prob8 should be 1 byte (u8)");
        assert_eq!(std::mem::size_of::<Dist2>(), 8, "Dist2 should be 8 bytes (i64)");
    }

    #[test]
    fn type_ranges() {
        // Verify range examples work as documented
        let _max_distance_cm: DistCm = i32::MAX; // ~21,474 km
        let _max_speed_cms: SpeedCms = i32::MAX; // ~21,474 km/h
        let _heading_north: HeadCdeg = 0; // 0°
        let _heading_east: HeadCdeg = 9000; // 90° in centidegrees
        let _certain: Prob8 = 255; // 100% probability
        let _impossible: Prob8 = 0; // 0% probability
        let _squared_distance: Dist2 = 1_000_000_i64; // (1000 cm)²
    }
}
