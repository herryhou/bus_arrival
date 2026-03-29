// JSON input structures for the preprocessor
//
// Defines the expected JSON format for route and stops input files.

use serde::{Deserialize, Serialize};

/// Input structure for route.json file
///
/// Contains a sequence of GPS coordinates representing the bus route.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteInput {
    /// Route GPS coordinates as [lat, lon]
    pub route_points: Vec<RoutePoint>,
}

/// A single route point [lat, lon]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoutePoint(pub f64, pub f64);

impl RoutePoint {
    pub fn lat(&self) -> f64 { self.0 }
    pub fn lon(&self) -> f64 { self.1 }
}

/// Input structure for stops.json file
///
/// Contains bus stop locations for projection onto the route.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StopsInput {
    /// Array of bus stop locations
    pub stops: Vec<StopLocation>,
}

/// GPS coordinates of a bus stop
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StopLocation {
    /// Latitude in decimal degrees
    pub lat: f64,
    /// Longitude in decimal degrees
    pub lon: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_route_json() {
        let json = r#"{
            "route_points": [
                [25.00425, 121.28645],
                [25.00566, 121.28619]
            ]
        }"#;

        let route: RouteInput = serde_json::from_str(json).expect("Failed to parse route JSON");

        assert_eq!(route.route_points.len(), 2);
        assert_eq!(route.route_points[0].lat(), 25.00425);
        assert_eq!(route.route_points[0].lon(), 121.28645);
    }

    #[test]
    fn parse_stops_json() {
        let json = r#"{"stops": [{"lat": 25.004283, "lon": 121.286559}]}"#;

        let stops: StopsInput = serde_json::from_str(json).expect("Failed to parse stops JSON");

        assert_eq!(stops.stops.len(), 1);
        assert_eq!(stops.stops[0].lat, 25.004283);
        assert_eq!(stops.stops[0].lon, 121.286559);
    }
}
