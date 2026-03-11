// JSON input structures for the preprocessor
//
// Defines the expected JSON format for route and stops input files.

use serde::{Deserialize, Serialize};

/// Input structure for route.json file
///
/// Contains a sequence of GPS coordinates representing the bus route.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteInput {
    /// Route GPS coordinates
    pub route_points: Vec<RoutePoint>,
}

/// A single route point with OSM data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoutePoint {
    /// Latitude in decimal degrees
    pub lat: f64,
    /// Longitude in decimal degrees
    pub lon: f64,
    /// OSM way ID
    pub way_id: u32,
    /// OSM node ID
    pub node_id: u32,
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
                {"lat": 25.00425, "lon": 121.28645, "way_id": 12345, "node_id": 67890},
                {"lat": 25.00566, "lon": 121.28619, "way_id": 12346, "node_id": 67891}
            ]
        }"#;

        let route: RouteInput = serde_json::from_str(json).expect("Failed to parse route JSON");

        assert_eq!(route.route_points.len(), 2);
        assert_eq!(route.route_points[0].lat, 25.00425);
        assert_eq!(route.route_points[0].lon, 121.28645);
        assert_eq!(route.route_points[0].way_id, 12345);
        assert_eq!(route.route_points[0].node_id, 67890);
    }

    #[test]
    fn parse_stops_json() {
        let json = r#"{"stops": [{"lat": 25.004283, "lon": 121.286559}]}"#;

        let stops: StopsInput = serde_json::from_str(json).expect("Failed to parse stops JSON");

        assert_eq!(stops.stops.len(), 1);
        assert_eq!(stops.stops[0].lat, 25.004283);
        assert_eq!(stops.stops[0].lon, 121.286559);
    }

    #[test]
    fn parse_route_empty() {
        let json = r#"{"route_points": []}"#;

        let route: RouteInput = serde_json::from_str(json).expect("Failed to parse empty route JSON");

        assert_eq!(route.route_points.len(), 0);
    }

    #[test]
    fn parse_stops_empty() {
        let json = r#"{"stops": []}"#;

        let stops: StopsInput = serde_json::from_str(json).expect("Failed to parse empty stops JSON");

        assert_eq!(stops.stops.len(), 0);
    }

    #[test]
    fn parse_multiple_stops() {
        let json = r#"{
            "stops": [
                {"lat": 25.004283, "lon": 121.286559},
                {"lat": 25.005000, "lon": 121.287000},
                {"lat": 25.006000, "lon": 121.288000}
            ]
        }"#;

        let stops: StopsInput = serde_json::from_str(json).expect("Failed to parse multiple stops JSON");

        assert_eq!(stops.stops.len(), 3);
        assert_eq!(stops.stops[0].lat, 25.004283);
        assert_eq!(stops.stops[1].lat, 25.005000);
        assert_eq!(stops.stops[2].lat, 25.006000);
    }

    #[test]
    fn serialize_route_input() {
        let route = RouteInput {
            route_points: vec![RoutePoint {
                lat: 25.00425,
                lon: 121.28645,
                way_id: 12345,
                node_id: 67890,
            }],
        };

        let json = serde_json::to_string(&route).expect("Failed to serialize route");

        assert!(json.contains("route_points"));
        assert!(json.contains("25.00425"));
        assert!(json.contains("121.28645"));
    }

    #[test]
    fn serialize_stops_input() {
        let stops = StopsInput {
            stops: vec![StopLocation {
                lat: 25.004283,
                lon: 121.286559,
            }],
        };

        let json = serde_json::to_string(&stops).expect("Failed to serialize stops");

        assert!(json.contains("stops"));
        assert!(json.contains("25.004283"));
        assert!(json.contains("121.286559"));
    }
}
