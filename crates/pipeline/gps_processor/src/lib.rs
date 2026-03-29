pub mod route_data;
pub mod nmea;
pub mod map_match;
pub mod kalman;
pub mod output;

// Re-export commonly used types
pub use nmea::NmeaState;
pub use kalman::{ProcessResult, process_gps_update, V_MAX_CMS, SIGMA_GPS_CM};
