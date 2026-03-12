//! Binary route data reader

use shared::{RouteNode, Stop, GridOrigin, SpatialGrid};
use std::io::{self, Read};
use std::fs::File;
use std::path::Path;

use crate::grid::build_spatial_grid;

pub const MAGIC: u32 = 0x42555341;
pub const VERSION: u16 = 1;

pub struct RouteData {
    pub nodes: Vec<RouteNode>,
    pub stops: Vec<Stop>,
    pub grid_origin: GridOrigin,
    pub grid: SpatialGrid,
}

/// Load route data from binary file
pub fn load_route_data(path: &Path) -> Result<RouteData, io::Error> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Validate minimum file size
    // Header: magic(4) + version(2) + node_count(2) + stop_count(1) + x0_cm(4) + y0_cm(4) = 17 bytes
    const MIN_HEADER_SIZE: usize = 17;
    if buffer.len() < MIN_HEADER_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("File too small: {} bytes (minimum {})", buffer.len(), MIN_HEADER_SIZE),
        ));
    }

    // Also validate we have enough bytes for declared node_count and stop_count
    let node_count = u16::from_le_bytes(buffer[6..8].try_into().unwrap()) as usize;
    let stop_count = buffer[8] as usize;

    let expected_size = MIN_HEADER_SIZE + (node_count * 52) + (stop_count * 12) + 4; // +4 for CRC32
    if buffer.len() < expected_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("File too small for declared counts: {} bytes (expected {})", buffer.len(), expected_size),
        ));
    }

    // Verify magic
    let magic = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
    if magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid magic: {:08x}", magic),
        ));
    }

    // Read version
    let version = u16::from_le_bytes(buffer[4..6].try_into().unwrap());
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported version: {}", version),
        ));
    }

    // Read header
    let x0_cm = i32::from_le_bytes(buffer[9..13].try_into().unwrap());
    let y0_cm = i32::from_le_bytes(buffer[13..17].try_into().unwrap());

    let grid_origin = GridOrigin { x0_cm, y0_cm };

    // Read route nodes (52 bytes each)
    let mut nodes = Vec::with_capacity(node_count);
    let mut offset = 17;
    for _ in 0..node_count {
        let node_bytes = &buffer[offset..offset + 52];
        unsafe {
            let node_ptr = node_bytes.as_ptr() as *const RouteNode;
            nodes.push(std::ptr::read(node_ptr));
        }
        offset += 52;
    }

    // Read stops (12 bytes each)
    let mut stops = Vec::with_capacity(stop_count);
    for _ in 0..stop_count {
        let stop_bytes = &buffer[offset..offset + 12];
        unsafe {
            let stop_ptr = stop_bytes.as_ptr() as *const Stop;
            stops.push(std::ptr::read(stop_ptr));
        }
        offset += 12;
    }

    // TODO: Read CRC32 and verify
    // Build spatial grid from nodes (Task 7)
    let grid = build_spatial_grid(&nodes, &grid_origin);

    Ok(RouteData {
        nodes,
        stops,
        grid_origin,
        grid,
    })
}
