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

/// Route node representing a point in a bus route.
///
/// # Memory Layout (52 bytes total)
///
/// ```text
/// Offset  Field           Type    Size
/// ------  --------------  ----    ----
/// 0       len2_cm2        i64     8
/// 8       line_c          i64     8
/// 16      x_cm            i32     4
/// 20      y_cm            i32     4
/// 24      cum_dist_cm     i32     4
/// 28      dx_cm           i32     4
/// 32      dy_cm           i32     4
/// 36      seg_len_cm      i32     4
/// 40      line_a          i32     4
/// 44      line_b          i32     4
/// 48      heading_cdeg    i16     2
/// 50      _pad            i16     2
/// ------                        ----
/// TOTAL                        52
/// ```
///
/// # Layout Rationale
/// - **i64 fields first** (offsets 0-15): Prevents padding between 8-byte and 4-byte fields
/// - **i32 fields next** (offsets 16-47): Natural alignment for 32-bit integers
/// - **i16 fields last** (offsets 48-51): Minimal padding for size alignment
/// - Total size: 2×8 + 8×4 + 2×2 = 52 bytes
///
/// # Field Descriptions
/// - `len2_cm2`: Squared length from previous node (cm²)
/// - `line_c`: Line equation constant term (for perpendicular distance calculation)
/// - `x_cm`, `y_cm`: Node coordinates in centimeters
/// - `cum_dist_cm`: Cumulative distance from route start (cm)
/// - `dx_cm`, `dy_cm`: Direction vector to next node (cm)
/// - `seg_len_cm`: Length of segment to next node (cm)
/// - `line_a`, `line_b`: Line equation coefficients (Ax + By + C = 0)
/// - `heading_cdeg`: Direction in centidegrees (0.01° units)
/// - `_pad`: Reserved for future use
///
/// # Embedded Compatibility
/// - `#[repr(C, packed)]` ensures packed layout without trailing padding
/// - Explicit padding field makes the layout transparent
/// - Suitable for direct serialization/deserialization
/// - Note: Packed structs may have unaligned access on some platforms
#[repr(C, packed)]
pub struct RouteNode {
    /// Squared distance from previous node (cm²)
    pub len2_cm2: i64,

    /// Line equation constant term (for perpendicular distance)
    pub line_c: i64,

    /// X coordinate (cm)
    pub x_cm: i32,

    /// Y coordinate (cm)
    pub y_cm: i32,

    /// Cumulative distance from route start (cm)
    pub cum_dist_cm: i32,

    /// X direction to next node (cm)
    pub dx_cm: i32,

    /// Y direction to next node (cm)
    pub dy_cm: i32,

    /// Length of segment to next node (cm)
    pub seg_len_cm: i32,

    /// Line equation coefficient A (for Ax + By + C = 0)
    pub line_a: i32,

    /// Line equation coefficient B
    pub line_b: i32,

    /// Heading in centidegrees (0.01° units)
    pub heading_cdeg: i16,

    /// Padding to ensure 52-byte total size
    pub _pad: i16,
}

const _: () = {
    // Compile-time assertion: RouteNode must be exactly 52 bytes
    let _ = [(); 52 - std::mem::size_of::<RouteNode>()];
};

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

    #[test]
    fn route_node_size() {
        // Verify RouteNode is exactly 52 bytes
        assert_eq!(
            std::mem::size_of::<RouteNode>(),
            52,
            "RouteNode should be exactly 52 bytes"
        );
        // With packed, alignment is 1
        assert_eq!(
            std::mem::align_of::<RouteNode>(),
            1,
            "RouteNode should have 1-byte alignment (packed)"
        );
    }

    #[test]
    fn route_node_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        // i64 fields at offsets 0-15
        assert_eq!(offset_of!(RouteNode, len2_cm2), 0, "len2_cm2 should be at offset 0");
        assert_eq!(offset_of!(RouteNode, line_c), 8, "line_c should be at offset 8");

        // i32 fields at offsets 16-47
        assert_eq!(offset_of!(RouteNode, x_cm), 16, "x_cm should be at offset 16");
        assert_eq!(offset_of!(RouteNode, y_cm), 20, "y_cm should be at offset 20");
        assert_eq!(
            offset_of!(RouteNode, cum_dist_cm),
            24,
            "cum_dist_cm should be at offset 24"
        );
        assert_eq!(offset_of!(RouteNode, dx_cm), 28, "dx_cm should be at offset 28");
        assert_eq!(offset_of!(RouteNode, dy_cm), 32, "dy_cm should be at offset 32");
        assert_eq!(
            offset_of!(RouteNode, seg_len_cm),
            36,
            "seg_len_cm should be at offset 36"
        );
        assert_eq!(offset_of!(RouteNode, line_a), 40, "line_a should be at offset 40");
        assert_eq!(offset_of!(RouteNode, line_b), 44, "line_b should be at offset 44");

        // i16 fields at offsets 48-51
        assert_eq!(
            offset_of!(RouteNode, heading_cdeg),
            48,
            "heading_cdeg should be at offset 48"
        );
        assert_eq!(offset_of!(RouteNode, _pad), 50, "_pad should be at offset 50");
    }

    #[test]
    fn route_node_default() {
        // Verify RouteNode can be created and initialized
        let node = RouteNode {
            len2_cm2: 10000,
            line_c: -500,
            x_cm: 123456,
            y_cm: 789012,
            cum_dist_cm: 1000,
            dx_cm: 100,
            dy_cm: 50,
            seg_len_cm: 112,
            line_a: 1,
            line_b: -2,
            heading_cdeg: 4500, // 45° in centidegrees
            _pad: 0,
        };

        // Copy fields to avoid creating references to packed struct
        let len2 = node.len2_cm2;
        let x = node.x_cm;
        let heading = node.heading_cdeg;

        assert_eq!(len2, 10000);
        assert_eq!(x, 123456);
        assert_eq!(heading, 4500);
    }
}
