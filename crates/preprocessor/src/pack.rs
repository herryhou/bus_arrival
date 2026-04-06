// Binary packing: serialize route data to route_data.bin format.
//
// Delegates to shared::binfile for the actual serialization logic.

use shared::{RouteNode, Stop, SpatialGrid};
use shared::binfile::pack_route_data;
use std::io::{self, Write};

/// Pack route data into binary format.
pub fn pack_v8_route_data(
    route_nodes: &[RouteNode],
    stops: &[Stop],
    grid: &SpatialGrid,
    lat_avg_deg: f64,
    output: &mut impl Write,
) -> io::Result<()> {
    pack_route_data(
        route_nodes,
        stops,
        grid,
        lat_avg_deg,
        output
    ).map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))
}
