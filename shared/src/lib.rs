// Semantic type aliases for GPS bus arrival detection system
//
// These type aliases provide domain-specific meaning to primitive types,
// making the code more self-documenting and preventing unit errors.

/// Magic bytes for route_data.bin: "BUSA" (BUS Arrival)
pub const MAGIC: u32 = 0x42555341;

/// Format version
pub const VERSION: u16 = 1;

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

/// Bus stop with corridor boundaries for arrival detection.
///
/// # Memory Layout (12 bytes total)
///
/// ```text
/// Offset  Field               Type    Size
/// ------  ------------------  ----    ----
/// 0       progress_cm        i32     4
/// 4       corridor_start_cm  i32     4
/// 8       corridor_end_cm    i32     4
/// ------                              ----
/// TOTAL                              12
/// ```
///
/// # Corridor Dimensions
/// - **L_pre = 80m**: Corridor extends 8000 cm behind the stop
/// - **L_post = 40m**: Corridor extends 4000 cm ahead of the stop
/// - Total corridor length: 120m (12000 cm)
///
/// # Field Descriptions
/// - `progress_cm`: Distance from route origin to stop position (cm)
/// - `corridor_start_cm`: Distance to start of detection corridor (cm)
/// - `corridor_end_cm`: Distance to end of detection corridor (cm)
///
/// # Invariants
/// - `corridor_start_cm < progress_cm < corridor_end_cm`
/// - The stop is positioned within the corridor, not at the center
/// - Typical: start = progress - 8000, end = progress + 4000
///
/// # Embedded Compatibility
/// - `#[repr(C)]` ensures stable layout across platforms
/// - No padding required (3× i32 = 12 bytes, naturally aligned)
/// - Suitable for direct serialization/deserialization
#[repr(C)]
pub struct Stop {
    /// Distance from route origin to stop (cm)
    pub progress_cm: DistCm,

    /// Start of detection corridor (cm, typically progress - 8000)
    pub corridor_start_cm: DistCm,

    /// End of detection corridor (cm, typically progress + 4000)
    pub corridor_end_cm: DistCm,
}

/// Grid origin for relative coordinate system.
///
/// # Memory Layout (8 bytes total)
///
/// ```text
/// Offset  Field    Type    Size
/// ------  -------  ----    ----
/// 0       x0_cm    i32     4
/// 4       y0_cm    i32     4
/// ------                  ----
/// TOTAL                  8
/// ```
///
/// # Purpose
/// Defines the FIXED origin (0, 0) point for a relative coordinate system.
/// All routes use the same origin at (120.0°E, 20.0°N) to ensure:
/// - Unified coordinate system across all routes
/// - Simpler implementation (no bbox computation needed)
/// - Safe from i32 overflow for Taiwan routes
///
/// # Fixed Origin Values (computed at compile time)
/// - `x0_cm`: 120.0°E = 1,253,868,624 cm (~12,539 km from prime meridian at 20°N)
/// - `y0_cm`: 20.0°N = 222,639,208 cm (~2,226 km from equator)
///
/// # Use Cases
/// - **Coordinate compression**: Reduce magnitude of coordinate values
///   while maintaining precision
/// - **Overflow prevention**: Taiwan coordinates fit within ±2,000 km
/// - **Cross-route consistency**: All routes share same reference point
///
/// # Field Descriptions
/// - `x0_cm`: Fixed origin X at 120.0°E (cm)
/// - `y0_cm`: Fixed origin Y at 20.0°N (cm)
///
/// # Example
/// ```rust
/// # use shared::{GridOrigin, DistCm};
/// // Fixed origin - same for all routes
/// let origin = GridOrigin {
///     x0_cm: 1_258_772_027,  // 120.0°E
///     y0_cm:   222_639_208,  // 20.0°N
/// };
/// let absolute_x: DistCm = 1_258_872_027; // ~120.001°E in cm
/// let relative_x = absolute_x - origin.x0_cm; // 100,000 cm (1 km)
/// ```
///
/// # Embedded Compatibility
/// - `#[repr(C)]` ensures stable layout across platforms
/// - No padding required (2× i32 = 8 bytes, naturally aligned)
/// - Suitable for direct serialization/deserialization
#[repr(C)]
pub struct GridOrigin {
    /// Fixed origin X: 120.0°E in centimeters
    pub x0_cm: DistCm,

    /// Fixed origin Y: 20.0°N in centimeters
    pub y0_cm: DistCm,
}

/// Parsed GPS data from NMEA sentences.
///
/// # Memory Layout (32 bytes total)
///
/// Note: Rust's default layout reorders fields for optimal alignment.
///
/// ```text
/// Offset  Field            Type        Size
/// ------  ---------------  ----------  ----
/// 0       lat              f64         8
/// 8       lon              f64         8
/// 16      speed_cms        SpeedCms    4
/// 20      heading_cdeg     HeadCdeg    2
/// 22      hdop_x10         u16         2
/// 24      has_fix          bool        1
/// 25-31   (padding)        -           7
/// ------                              ----
/// TOTAL                              32
/// ```
///
/// # Field Descriptions
/// - `lat`: Latitude in degrees WGS84 (negative for South)
/// - `lon`: Longitude in degrees WGS84 (negative for West)
/// - `heading_cdeg`: Heading in centidegrees (0.01° units, 0-36000)
/// - `speed_cms`: Speed in centimeters per second
/// - `hdop_x10`: Horizontal dilution of precision × 10 (HDOP × 10)
/// - `has_fix`: Whether GPS has a valid fix
///
/// # Use Cases
/// - **Input to localization pipeline**: Raw GPS data from NMEA sentences
/// - **Quality filtering**: Use `hdop_x10` and `has_fix` to filter poor quality data
/// - **Speed estimation**: Used for Kalman filter and outlier detection
/// - **Direction validation**: Compare `heading_cdeg` with route heading
///
/// # Example
/// ```rust
/// # use shared::GpsPoint;
/// let mut point = GpsPoint::new();
/// // Parse from NMEA...
/// point.lat = 25.0478;
/// point.lon = 121.5170;
/// point.heading_cdeg = 4500; // 45° in centidegrees
/// point.speed_cms = 500;     // 5 m/s in cm/s
/// point.hdop_x10 = 12;       // HDOP 1.2
/// point.has_fix = true;
/// ```
#[derive(Debug, Clone)]
pub struct GpsPoint {
    /// Latitude in degrees WGS84
    pub lat: f64,

    /// Longitude in degrees WGS84
    pub lon: f64,

    /// Heading in centidegrees (0.01° units, 0-36000)
    pub heading_cdeg: HeadCdeg,

    /// Speed in centimeters per second
    pub speed_cms: SpeedCms,

    /// HDOP × 10 (Horizontal dilution of precision)
    pub hdop_x10: u16,

    /// Valid GPS fix
    pub has_fix: bool,
}

impl GpsPoint {
    /// Creates a new GpsPoint with default values.
    ///
    /// # Returns
    ///
    /// A GpsPoint with all fields set to zero/false:
    /// - `lat`: 0.0
    /// - `lon`: 0.0
    /// - `heading_cdeg`: 0
    /// - `speed_cms`: 0
    /// - `hdop_x10`: 0
    /// - `has_fix`: false
    ///
    /// # Example
    ///
    /// ```rust
    /// # use shared::GpsPoint;
    /// let point = GpsPoint::new();
    /// assert_eq!(point.lat, 0.0);
    /// assert_eq!(point.lon, 0.0);
    /// assert_eq!(point.heading_cdeg, 0);
    /// assert_eq!(point.speed_cms, 0);
    /// assert_eq!(point.hdop_x10, 0);
    /// assert_eq!(point.has_fix, false);
    /// ```
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
///
/// # Memory Layout (8 bytes total)
///
/// ```text
/// Offset  Field    Type      Size
/// ------  -------  --------  ----
/// 0       s_cm     DistCm    4
/// 4       v_cms    SpeedCms  4
/// ------                      ----
/// TOTAL                      8
/// ```
///
/// # Purpose
/// Implements a 1D Kalman filter for tracking route progress (distance along route)
/// with fixed-point arithmetic suitable for embedded systems. The filter combines:
/// - **Prediction step**: Uses velocity estimate to predict next position
/// - **Update step**: Incorporates GPS measurements with adaptive gains
///
/// # Fixed-Point Design
/// Uses integer arithmetic with division by 256 for Kalman gains:
/// - `Ks = 51/256 ≈ 0.20`: Position gain (varies with HDOP)
/// - `Kv = 77/256 ≈ 0.30`: Velocity gain (fixed)
///
/// # HDOP-Adaptive Filtering
/// The position gain (Ks) adapts to GPS quality (HDOP):
/// - **HDOP ≤ 2.0**: Ks = 77/256 ≈ 0.30 (high trust in GPS)
/// - **HDOP ≤ 3.0**: Ks = 51/256 ≈ 0.20 (moderate trust)
/// - **HDOP ≤ 5.0**: Ks = 26/256 ≈ 0.10 (low trust)
/// - **HDOP > 5.0**: Ks = 13/256 ≈ 0.05 (very low trust)
///
/// # Field Descriptions
/// - `s_cm`: Route progress estimate in centimeters from route origin
/// - `v_cms`: Speed estimate in centimeters per second
///
/// # Example
/// ```rust
/// # use shared::KalmanState;
/// let mut state = KalmanState::new();
/// // Initial GPS measurement at 100m with 5 m/s speed
/// state.update(10000, 500); // z=10000cm, v=500cm/s
/// // Subsequent measurement with HDOP 2.5 (×10 = 25)
/// state.update_adaptive(10100, 500, 25);
/// ```
///
/// # Embedded Compatibility
/// - `#[repr(C)]` ensures stable layout across platforms
/// - No padding required (2× i32 = 8 bytes, naturally aligned)
/// - Fixed-point arithmetic avoids floating-point operations
#[repr(C)]
#[derive(Debug, Clone)]
pub struct KalmanState {
    /// Route progress estimate (cm from route origin)
    pub s_cm: DistCm,

    /// Speed estimate (cm/s)
    pub v_cms: SpeedCms,
}

impl KalmanState {
    /// Creates a new KalmanState with initial estimates at zero.
    ///
    /// # Returns
    ///
    /// A KalmanState with:
    /// - `s_cm`: 0 (at route origin)
    /// - `v_cms`: 0 (stationary)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use shared::KalmanState;
    /// let state = KalmanState::new();
    /// assert_eq!(state.s_cm, 0);
    /// assert_eq!(state.v_cms, 0);
    /// ```
    pub fn new() -> Self {
        KalmanState { s_cm: 0, v_cms: 0 }
    }

    /// Fixed-point Kalman filter update with default gains.
    ///
    /// Uses fixed Kalman gains: Ks = 51/256 ≈ 0.20, Kv = 77/256 ≈ 0.30
    ///
    /// # Algorithm
    /// 1. Predict: `s_pred = s_cm + v_cms`
    /// 2. Update position: `s_cm = s_pred + Ks * (z_cm - s_pred)`
    /// 3. Update velocity: `v_cms = v_cms + Kv * (v_gps_cms - v_cms)`
    ///
    /// # Parameters
    /// - `z_cm`: GPS measurement of route progress (cm)
    /// - `v_gps_cms`: GPS speed measurement (cm/s)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use shared::KalmanState;
    /// let mut state = KalmanState::new();
    /// state.update(10000, 500); // GPS at 100m, 5 m/s
    /// assert!(state.s_cm > 0);
    /// assert!(state.v_cms > 0);
    /// ```
    pub fn update(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms) {
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (51 * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }

    /// HDOP-adaptive Kalman filter update.
    ///
    /// Adapts position gain (Ks) based on GPS quality (HDOP), while keeping
    /// velocity gain fixed at Kv = 77/256 ≈ 0.30.
    ///
    /// # Parameters
    /// - `z_cm`: GPS measurement of route progress (cm)
    /// - `v_gps_cms`: GPS speed measurement (cm/s)
    /// - `hdop_x10`: HDOP × 10 (e.g., HDOP 1.5 → hdop_x10 = 15)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use shared::KalmanState;
    /// let mut state = KalmanState::new();
    /// // High quality GPS (HDOP 1.2)
    /// state.update_adaptive(10000, 500, 12);
    /// // Low quality GPS (HDOP 6.0)
    /// state.update_adaptive(10100, 500, 60);
    /// ```
    pub fn update_adaptive(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms, hdop_x10: u16) {
        let ks = Self::ks_from_hdop(hdop_x10);
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (ks * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }

    /// Computes position Kalman gain from HDOP value.
    ///
    /// # Parameters
    /// - `hdop_x10`: HDOP × 10 (e.g., HDOP 2.5 → hdop_x10 = 25)
    ///
    /// # Returns
    ///
    /// Kalman gain numerator (divide by 256 for actual gain):
    /// - **HDOP ≤ 2.0**: 77 (gain ≈ 0.30)
    /// - **HDOP ≤ 3.0**: 51 (gain ≈ 0.20)
    /// - **HDOP ≤ 5.0**: 26 (gain ≈ 0.10)
    /// - **HDOP > 5.0**: 13 (gain ≈ 0.05)
    ///
    /// # Note
    ///
    /// This is a private helper function used internally by `update_adaptive`.
    /// The adaptive gain behavior is tested through `update_adaptive`.
    fn ks_from_hdop(hdop_x10: u16) -> i32 {
        match hdop_x10 {
            0..=20 => 77,   // HDOP ≤ 2.0 → Ks ≈ 0.30
            21..=30 => 51,  // HDOP ≤ 3.0 → Ks ≈ 0.20
            31..=50 => 26,  // HDOP ≤ 5.0 → Ks ≈ 0.10
            _ => 13,        // HDOP > 5.0 → Ks ≈ 0.05
        }
    }
}

/// Spatial grid for O(k) map matching.
///
/// # Memory Layout
///
/// ```text
/// Offset  Field        Type      Size
/// ------  -----------  --------  ----
/// 0       cells        Vec       24
/// 24      grid_size_cm DistCm    4
/// 28      cols         u32       4
/// 32      rows         u32       4
/// 36      x0_cm        DistCm    4
/// 40      y0_cm        DistCm    4
/// ------                          ----
/// TOTAL                   44 + vec data
/// ```
///
/// **Note**: Vec internally contains a pointer, capacity, and length (3×8=24 bytes),
/// plus heap-allocated data for the actual vector contents.
///
/// # Purpose
/// Implements a spatial index for fast map matching. The grid divides the map
/// into 100m × 100m cells, allowing O(k) neighborhood queries instead of O(n)
/// linear search, where k is the number of route nodes in nearby cells.
///
/// # Grid Dimensions
/// - **Cell size**: 10000 cm (100m) - optimized for typical GPS accuracy
/// - **Query radius**: 3×3 cell neighborhood (300m × 300m)
/// - **Coordinate system**: Uses fixed origin at (120.0°E, 20.0°N)
///
/// # Field Descriptions
/// - `cells`: 2D grid flattened to 1D vector, each cell contains node indices
/// - `grid_size_cm`: Cell dimension (10000 cm = 100m)
/// - `cols`: Number of grid columns (x-axis)
/// - `rows`: Number of grid rows (y-axis)
/// - `x0_cm`: Grid origin X coordinate (cm)
/// - `y0_cm`: Grid origin Y coordinate (cm)
///
/// # Use Cases
/// - **Map matching**: Find nearby route nodes for GPS position
/// - **Neighborhood queries**: Get all nodes within 3×3 cell area
/// - **Spatial indexing**: Accelerate candidate selection
///
/// # Example
/// ```rust
/// # use shared::{SpatialGrid, DistCm};
/// let grid = SpatialGrid {
///     cells: vec![vec![0, 1], vec![2, 3]], // 2×2 grid
///     grid_size_cm: 10000,  // 100m cells
///     cols: 2,
///     rows: 2,
///     x0_cm: 1253868624,    // 120.0°E
///     y0_cm: 222639208,     // 20.0°N
/// };
/// ```
///
/// # Notes
/// - Methods for querying and building the grid are implemented in
///   `simulator/src/grid.rs` to avoid circular dependencies
/// - The grid uses the same fixed origin as `GridOrigin` for consistency
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    /// 2D grid flattened: cells[row * cols + col] contains node indices
    pub cells: Vec<Vec<usize>>,

    /// Grid cell size (10000 cm = 100m)
    pub grid_size_cm: DistCm,

    /// Number of columns in grid (x-axis)
    pub cols: u32,

    /// Number of rows in grid (y-axis)
    pub rows: u32,

    /// Grid origin X coordinate (cm, typically 120.0°E)
    pub x0_cm: DistCm,

    /// Grid origin Y coordinate (cm, typically 20.0°N)
    pub y0_cm: DistCm,
}

/// Dead-reckoning state for GPS outage compensation.
///
/// # Memory Layout (24 bytes total)
///
/// ```text
/// Offset  Field           Type        Size
/// ------  -------------  ---------  ----
/// 0       last_gps_time   Option<u64> 16
/// 16      last_valid_s    DistCm      4
/// 20      filtered_v      SpeedCms    4
/// ------                              ----
/// TOTAL                              24
/// ```
///
/// **Note**: `Option<u64>` is 16 bytes (8-byte discriminant + 8-byte value) due
/// to alignment requirements. The struct has 8-byte alignment from `Option<u64>`.
///
/// # Purpose
/// Maintains state for dead-reckoning (DR) mode during GPS outages. When GPS
/// signal is lost (tunnels, urban canyons), the system uses the last known
/// speed to estimate position changes: `s_est = s_last + v_filtered × Δt`.
///
/// # Dead-Reckoning Algorithm
/// 1. **Normal mode**: GPS available, update filtered speed with EMA
/// 2. **Outage mode**: Use last filtered speed to estimate position
/// 3. **Recovery**: Reset when valid GPS returns
///
/// # Field Descriptions
/// - `last_gps_time`: Timestamp of last valid GPS (seconds since epoch)
/// - `last_valid_s`: Last known route progress (cm from route origin)
/// - `filtered_v`: Exponentially smoothed speed estimate (cm/s)
///
/// # Speed Filtering
/// Uses exponential moving average (EMA) to smooth GPS speed measurements:
/// - Reduces impact of speed noise/outliers
/// - Provides stable estimate for DR extrapolation
/// - Formula: `v_filtered = α × v_new + (1-α) × v_filtered`
///
/// # Use Cases
/// - **Tunnel navigation**: Continue tracking when GPS is unavailable
/// - **Urban canyons**: Bridge short GPS gaps
/// - **Speed smoothing**: Reduce GPS speed measurement noise
///
/// # Example
/// ```rust
/// # use shared::DrState;
/// let mut dr = DrState::new();
/// // GPS update at t=100s
/// dr.last_gps_time = Some(100);
/// dr.last_valid_s = 10000;  // 100m along route
/// dr.filtered_v = 500;      // 5 m/s
/// // At t=105s (5s outage), estimate: s ≈ 10000 + 500×5 = 12500 cm
/// ```
///
/// # Embedded Compatibility
/// - Uses `Option<u64>` which requires 16 bytes (8-byte discriminant + 8-byte value)
/// - 8-byte alignment required for the Option<u64> field
/// - Total size is 24 bytes with 4-byte tail padding for alignment
/// - Suitable for embedded systems with sufficient memory
#[derive(Debug, Clone)]
pub struct DrState {
    /// Timestamp of last valid GPS fix (seconds since epoch, None if no fix yet)
    pub last_gps_time: Option<u64>,

    /// Last known route progress (cm from route origin)
    pub last_valid_s: DistCm,

    /// EMA-smoothed speed estimate (cm/s)
    pub filtered_v: SpeedCms,
}

impl DrState {
    /// Creates a new DrState with initial values.
    ///
    /// # Returns
    ///
    /// A DrState with:
    /// - `last_gps_time`: None (no GPS received yet)
    /// - `last_valid_s`: 0 (at route origin)
    /// - `filtered_v`: 0 (stationary)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use shared::DrState;
    /// let dr = DrState::new();
    /// assert_eq!(dr.last_gps_time, None);
    /// assert_eq!(dr.last_valid_s, 0);
    /// assert_eq!(dr.filtered_v, 0);
    /// ```
    pub fn new() -> Self {
        DrState {
            last_gps_time: None,
            last_valid_s: 0,
            filtered_v: 0,
        }
    }
}

const _: () = {
    // Compile-time assertions for struct sizes
    let _ = [(); 52 - std::mem::size_of::<RouteNode>()];
    let _ = [(); 12 - std::mem::size_of::<Stop>()];
    let _ = [(); 8 - std::mem::size_of::<GridOrigin>()];
    let _ = [(); 32 - std::mem::size_of::<GpsPoint>()];
    let _ = [(); 8 - std::mem::size_of::<KalmanState>()];
    // Note: DrState contains Option<u64> which is 24 bytes on 64-bit (8 + padding + 4 + 4)
    // Note: SpatialGrid contains Vec, so size is not compile-time constant
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

    #[test]
    fn stop_size() {
        // Verify Stop is exactly 12 bytes
        assert_eq!(
            std::mem::size_of::<Stop>(),
            12,
            "Stop should be exactly 12 bytes"
        );
        // Natural alignment for i32 fields
        assert_eq!(
            std::mem::align_of::<Stop>(),
            4,
            "Stop should have 4-byte alignment"
        );
    }

    #[test]
    fn stop_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        assert_eq!(
            offset_of!(Stop, progress_cm),
            0,
            "progress_cm should be at offset 0"
        );
        assert_eq!(
            offset_of!(Stop, corridor_start_cm),
            4,
            "corridor_start_cm should be at offset 4"
        );
        assert_eq!(
            offset_of!(Stop, corridor_end_cm),
            8,
            "corridor_end_cm should be at offset 8"
        );
    }

    #[test]
    fn stop_corridor_monotonic() {
        // Verify corridor invariant: start < progress < end
        let stop = Stop {
            progress_cm: 10000,
            corridor_start_cm: 2000,  // 10000 - 8000
            corridor_end_cm: 14000,   // 10000 + 4000
        };

        assert!(
            stop.corridor_start_cm < stop.progress_cm,
            "corridor_start_cm should be less than progress_cm"
        );
        assert!(
            stop.progress_cm < stop.corridor_end_cm,
            "progress_cm should be less than corridor_end_cm"
        );
    }

    #[test]
    fn grid_origin_size() {
        // Verify GridOrigin is exactly 8 bytes
        assert_eq!(
            std::mem::size_of::<GridOrigin>(),
            8,
            "GridOrigin should be exactly 8 bytes"
        );
        // Natural alignment for i32 fields
        assert_eq!(
            std::mem::align_of::<GridOrigin>(),
            4,
            "GridOrigin should have 4-byte alignment"
        );
    }

    #[test]
    fn grid_origin_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        assert_eq!(
            offset_of!(GridOrigin, x0_cm),
            0,
            "x0_cm should be at offset 0"
        );
        assert_eq!(
            offset_of!(GridOrigin, y0_cm),
            4,
            "y0_cm should be at offset 4"
        );
    }

    #[test]
    fn grid_origin_relative_coords() {
        // Verify relative coordinate calculation
        let origin = GridOrigin {
            x0_cm: 100000,
            y0_cm: 200000,
        };

        let abs_x: DistCm = 100500;
        let abs_y: DistCm = 200300;

        let rel_x = abs_x - origin.x0_cm;
        let rel_y = abs_y - origin.y0_cm;

        assert_eq!(rel_x, 500, "relative X should be 500 cm");
        assert_eq!(rel_y, 300, "relative Y should be 300 cm");
    }

    #[test]
    fn gpspoint_size() {
        // Verify GpsPoint is exactly 32 bytes
        assert_eq!(
            std::mem::size_of::<GpsPoint>(),
            32,
            "GpsPoint should be exactly 32 bytes"
        );
        // Natural alignment for f64 fields
        assert_eq!(
            std::mem::align_of::<GpsPoint>(),
            8,
            "GpsPoint should have 8-byte alignment"
        );
    }

    #[test]
    fn gpspoint_new() {
        // Verify GpsPoint::new() returns correct default values
        let point = GpsPoint::new();

        assert_eq!(point.lat, 0.0, "lat should be 0.0");
        assert_eq!(point.lon, 0.0, "lon should be 0.0");
        assert_eq!(point.heading_cdeg, 0, "heading_cdeg should be 0");
        assert_eq!(point.speed_cms, 0, "speed_cms should be 0");
        assert_eq!(point.hdop_x10, 0, "hdop_x10 should be 0");
        assert_eq!(point.has_fix, false, "has_fix should be false");
    }

    #[test]
    fn gpspoint_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        assert_eq!(offset_of!(GpsPoint, lat), 0, "lat should be at offset 0");
        assert_eq!(offset_of!(GpsPoint, lon), 8, "lon should be at offset 8");
        assert_eq!(
            offset_of!(GpsPoint, speed_cms),
            16,
            "speed_cms should be at offset 16"
        );
        assert_eq!(
            offset_of!(GpsPoint, heading_cdeg),
            20,
            "heading_cdeg should be at offset 20"
        );
        assert_eq!(
            offset_of!(GpsPoint, hdop_x10),
            22,
            "hdop_x10 should be at offset 22"
        );
        assert_eq!(
            offset_of!(GpsPoint, has_fix),
            24,
            "has_fix should be at offset 24"
        );
    }

    #[test]
    fn kalmanstate_size() {
        // Verify KalmanState is exactly 8 bytes
        assert_eq!(
            std::mem::size_of::<KalmanState>(),
            8,
            "KalmanState should be exactly 8 bytes"
        );
        // Natural alignment for i32 fields
        assert_eq!(
            std::mem::align_of::<KalmanState>(),
            4,
            "KalmanState should have 4-byte alignment"
        );
    }

    #[test]
    fn kalmanstate_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        assert_eq!(
            offset_of!(KalmanState, s_cm),
            0,
            "s_cm should be at offset 0"
        );
        assert_eq!(
            offset_of!(KalmanState, v_cms),
            4,
            "v_cms should be at offset 4"
        );
    }

    #[test]
    fn kalman_initial_state() {
        let state = KalmanState::new();
        assert_eq!(state.s_cm, 0);
        assert_eq!(state.v_cms, 0);
    }

    #[test]
    fn kalman_update_basic() {
        let mut state = KalmanState::new();
        state.update(10000, 500); // z=10000cm, v=500cm/s
        assert!(state.s_cm > 0);
        assert!(state.v_cms > 0);
    }

    #[test]
    fn kalman_smoothing() {
        let mut state = KalmanState::new();

        // Initialize filter at a known state
        state.s_cm = 10000;
        state.v_cms = 500;

        // Simulate consistent measurements
        // Raw GPS: 10500 (prediction + measurement)
        // Filtered: should be somewhere between prediction and measurement
        let s_before = state.s_cm;
        state.update(10500, 500);

        // The update combines prediction (s_cm + v_cms = 10500) with measurement (10500)
        // So result should be close to 10500 but smoothed
        assert!(state.s_cm > s_before, "State should increase");
        assert!(state.s_cm <= 10500, "State should not exceed measurement");
    }

    #[test]
    fn kalman_hdop_adaptive() {
        // Test that HDOP-adaptive update uses different gains
        let mut state1 = KalmanState::new();
        let mut state2 = KalmanState::new();
        let mut state3 = KalmanState::new();

        // Initialize both at same state
        state1.s_cm = 10000;
        state1.v_cms = 500;
        state2.s_cm = 10000;
        state2.v_cms = 500;
        state3.s_cm = 10000;
        state3.v_cms = 500;

        // Apply same measurement with different HDOP values
        // Prediction = 10000 + 500 = 10500
        // Measurement = 11000 (different from prediction to see gain effect)
        state1.update_adaptive(11000, 500, 15); // HDOP 1.5 - high trust (Ks = 77)
        state2.update_adaptive(11000, 500, 25); // HDOP 2.5 - medium trust (Ks = 51)
        state3.update_adaptive(11000, 500, 60); // HDOP 6.0 - low trust (Ks = 13)

        // Higher HDOP (lower quality) should result in less aggressive update
        // State with low trust should stay closer to prediction (10500) than state with high trust
        assert!(state3.s_cm < state2.s_cm, "Low trust should filter more (Ks=13 vs Ks=51)");
        assert!(state2.s_cm < state1.s_cm, "Medium trust should filter less than high trust (Ks=51 vs Ks=77)");
    }

    #[test]
    fn spatialgrid_can_create() {
        // Verify SpatialGrid can be instantiated
        let grid = SpatialGrid {
            cells: vec![vec![0, 1], vec![2, 3]],
            grid_size_cm: 10000,
            cols: 2,
            rows: 2,
            x0_cm: 1253868624,
            y0_cm: 222639208,
        };

        assert_eq!(grid.grid_size_cm, 10000);
        assert_eq!(grid.cols, 2);
        assert_eq!(grid.rows, 2);
        assert_eq!(grid.cells.len(), 2); // 2 rows
    }

    #[test]
    fn spatialgrid_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        let _grid = SpatialGrid {
            cells: vec![],
            grid_size_cm: 10000,
            cols: 2,
            rows: 2,
            x0_cm: 100000,
            y0_cm: 200000,
        };

        // Vec is at offset 0
        assert_eq!(offset_of!(SpatialGrid, cells), 0, "cells should be at offset 0");
        // After Vec (24 bytes on 64-bit), DistCm at offset 24
        assert_eq!(offset_of!(SpatialGrid, grid_size_cm), 24, "grid_size_cm should be at offset 24");
        assert_eq!(offset_of!(SpatialGrid, cols), 28, "cols should be at offset 28");
        assert_eq!(offset_of!(SpatialGrid, rows), 32, "rows should be at offset 32");
        assert_eq!(offset_of!(SpatialGrid, x0_cm), 36, "x0_cm should be at offset 36");
        assert_eq!(offset_of!(SpatialGrid, y0_cm), 40, "y0_cm should be at offset 40");
    }

    #[test]
    fn drstate_size() {
        // Verify DrState size
        // On 64-bit: Option<u64> = 8 bytes, DistCm = 4, SpeedCms = 4
        // With alignment: Option<u64> (8) + padding (4) + DistCm (4) + SpeedCms (4) = 24 bytes
        // Or: Option<u64> (8) + DistCm (4) + SpeedCms (4) + padding (8) = 24 bytes
        assert_eq!(
            std::mem::size_of::<DrState>(),
            24,
            "DrState should be 24 bytes"
        );
        // Natural alignment for u64
        assert_eq!(
            std::mem::align_of::<DrState>(),
            8,
            "DrState should have 8-byte alignment (u64)"
        );
    }

    #[test]
    fn drstate_new() {
        // Verify DrState::new() returns correct default values
        let dr = DrState::new();

        assert_eq!(dr.last_gps_time, None, "last_gps_time should be None");
        assert_eq!(dr.last_valid_s, 0, "last_valid_s should be 0");
        assert_eq!(dr.filtered_v, 0, "filtered_v should be 0");
    }

    #[test]
    fn drstate_field_offsets() {
        // Verify field offsets match the documented layout
        use std::mem::offset_of;

        // Option<u64> at offset 0
        assert_eq!(
            offset_of!(DrState, last_gps_time),
            0,
            "last_gps_time should be at offset 0"
        );
        // After Option<u64> (8 bytes), but Rust may add padding for alignment
        // Let's just check the relative ordering
        assert!(
            offset_of!(DrState, last_valid_s) > offset_of!(DrState, last_gps_time),
            "last_valid_s should be after last_gps_time"
        );
        assert!(
            offset_of!(DrState, filtered_v) > offset_of!(DrState, last_valid_s),
            "filtered_v should be after last_valid_s"
        );
    }

    #[test]
    fn drstate_dead_reckoning_estimation() {
        // Verify dead-reckoning position estimation logic
        let mut dr = DrState::new();

        // GPS update at t=100s
        dr.last_gps_time = Some(100);
        dr.last_valid_s = 10000;  // 100m along route
        dr.filtered_v = 500;      // 5 m/s

        // After 5s outage (t=105s), estimate position
        // s_est = s_last + v_filtered × Δt = 10000 + 500 × 5 = 12500 cm
        let delta_t = 5;
        let estimated_s = dr.last_valid_s + dr.filtered_v * delta_t;

        assert_eq!(estimated_s, 12500, "Estimated position should be 12500 cm");
    }

    #[test]
    fn drstate_gps_outage_detection() {
        // Verify GPS outage can be detected
        let mut dr = DrState::new();

        // No GPS yet
        assert_eq!(dr.last_gps_time, None);

        // GPS update received
        dr.last_gps_time = Some(100);
        assert_eq!(dr.last_gps_time, Some(100));

        // Can detect outage: current_time > last_gps_time + threshold
        let current_time = 110;  // 10 seconds later
        let is_outage = current_time > dr.last_gps_time.unwrap() + 5;
        assert!(is_outage, "Should detect GPS outage after 5 seconds");
    }
}
