//! Binary file format handling for bus arrival system.
//! 
//! Defines the MAGIC bytes, versioning, and zero-copy loading logic
//! used by both the preprocessor (for packing) and the runtime (for loading).

use crate::{RouteNode, Stop, SpatialGrid};
use core::marker::PhantomData;
use core::mem::size_of;

/// Magic bytes for route_data.bin: "BUSA" (BUS Arrival)
pub const MAGIC: u32 = 0x42555341;

/// Format version
pub const VERSION: u16 = 1;

/// Error types for the bus arrival binary file handling
#[derive(Debug, PartialEq)]
pub enum BusError {
    InvalidMagic,
    InvalidVersion,
    InvalidLength,
    ChecksumMismatch,
    OutOfBounds,
    IoError,
}

/// A read-only view into the spatial grid index.
/// Enables O(1) cell access directly from Flash memory.
pub struct SpatialGridView<'a> {
    pub cols: u32,
    pub rows: u32,
    pub grid_size_cm: i32,
    offsets_base: *const u8,
    data_base: *const u8,
    _marker: PhantomData<&'a u8>,
}

impl<'a> SpatialGridView<'a> {
    /// Returns the segment indices for a specific cell.
    pub fn get_cell(&self, col: u32, row: u32) -> Result<&'a [u16], BusError> {
        if col >= self.cols || row >= self.rows {
            return Err(BusError::OutOfBounds);
        }
        let idx = (row * self.cols + col) as usize;
        
        let offset_ptr = unsafe { self.offsets_base.add(idx * 4) as *const u32 };
        let start_offset = unsafe { core::ptr::read_unaligned(offset_ptr) } as usize;
        
        let cell_ptr = unsafe { self.data_base.add(start_offset) as *const u16 };
        // First u16 is the count
        let count = unsafe { core::ptr::read_unaligned(cell_ptr) } as usize;
        let indices_ptr = unsafe { cell_ptr.add(1) };
        
        // slice::from_raw_parts still requires alignment for the type.
        // If the base pointer is not aligned, we must use a different approach or 
        // ensure alignment during packing.
        // For now, we'll assume the caller provides an aligned buffer.
        Ok(unsafe { core::slice::from_raw_parts(indices_ptr, count) })
    }
}

/// The complete route data, referenced directly from a byte slice.
pub struct RouteData<'a> {
    pub x0_cm: i32,
    pub y0_cm: i32,
    /// Average latitude for projection (computed from route points)
    pub lat_avg_deg: f64,
    pub node_count: usize,
    pub stop_count: usize,
    nodes_ptr: *const RouteNode,
    stops_ptr: *const Stop,
    pub grid: SpatialGridView<'a>,
    pub gaussian_lut: &'a [u8; 256],
    pub logistic_lut: &'a [u8; 128],
}

impl<'a> RouteData<'a> {
    /// Get a specific route node by index.
    pub fn get_node(&self, index: usize) -> Option<RouteNode> {
        if index >= self.node_count { return None; }
        unsafe { Some(core::ptr::read_unaligned(self.nodes_ptr.add(index))) }
    }

    /// Get a specific stop by index.
    pub fn get_stop(&self, index: usize) -> Option<Stop> {
        if index >= self.stop_count { return None; }
        unsafe { Some(core::ptr::read_unaligned(self.stops_ptr.add(index))) }
    }

    /// Zero-copy load of RouteData from bytes.
    pub fn load(data: &'a [u8]) -> Result<Self, BusError> {
        if data.len() < 28 + 4 {
            return Err(BusError::InvalidLength);
        }

        let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let version = u16::from_le_bytes(data[4..6].try_into().unwrap());
        if magic != MAGIC { return Err(BusError::InvalidMagic); }
        if version != VERSION { return Err(BusError::InvalidVersion); }

        let node_count = u16::from_le_bytes(data[6..8].try_into().unwrap()) as usize;
        let stop_count = data[8] as usize;
        // data[9..12] is padding
        let x0_cm = i32::from_le_bytes(data[12..16].try_into().unwrap());
        let y0_cm = i32::from_le_bytes(data[16..20].try_into().unwrap());
        let lat_avg_deg = f64::from_le_bytes(data[20..28].try_into().unwrap());

        let received_crc = u32::from_le_bytes(data[data.len()-4..].try_into().unwrap());
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&data[..data.len()-4]);
        if hasher.finalize() != received_crc {
            return Err(BusError::ChecksumMismatch);
        }

        let mut offset = 28;

        let nodes_size = node_count * size_of::<RouteNode>();
        if data.len() < offset + nodes_size { return Err(BusError::InvalidLength); }
        let nodes_ptr = data[offset..].as_ptr() as *const RouteNode;
        offset += nodes_size;

        let stops_size = stop_count * size_of::<Stop>();
        if data.len() < offset + stops_size { return Err(BusError::InvalidLength); }
        let stops_ptr = data[offset..].as_ptr() as *const Stop;
        offset += stops_size;

        if data.len() < offset + 12 { return Err(BusError::InvalidLength); }
        let cols = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        let rows = u32::from_le_bytes(data[offset+4..offset+8].try_into().unwrap());
        let grid_size_cm = i32::from_le_bytes(data[offset+8..offset+12].try_into().unwrap());
        offset += 12;

        let cell_count = (cols * rows) as usize;
        let offsets_size = cell_count * 4;
        if data.len() < offset + offsets_size { return Err(BusError::InvalidLength); }
        let offsets_base = data[offset..].as_ptr();
        offset += offsets_size;

        let grid_data_start = offset;
        let luts_start = data.len() - 388;
        if luts_start < grid_data_start { return Err(BusError::InvalidLength); }
        
        let grid = SpatialGridView {
            cols,
            rows,
            grid_size_cm,
            offsets_base,
            data_base: data[grid_data_start..].as_ptr(),
            _marker: PhantomData,
        };

        let gaussian_lut = data[luts_start..luts_start+256].try_into().unwrap();
        let logistic_lut = data[luts_start+256..luts_start+384].try_into().unwrap();

        Ok(RouteData {
            x0_cm,
            y0_cm,
            lat_avg_deg,
            node_count,
            stop_count,
            nodes_ptr,
            stops_ptr,
            grid,
            gaussian_lut,
            logistic_lut,
        })
    }
}

/// Pack route data into binary format.
///
/// This function is intended for use in the preprocessor (requires std).
#[cfg(feature = "std")]
pub fn pack_route_data(
    route_nodes: &[RouteNode],
    stops: &[Stop],
    grid: &SpatialGrid,
    lat_avg_deg: f64,
    gaussian_lut: &[u8],
    logistic_lut: &[u8],
    output: &mut impl std::io::Write,
) -> Result<(), BusError> {
    use std::io::Write;

    let mut buffer = Vec::new();
    let node_count = route_nodes.len() as u16;
    let stop_count = stops.len() as u8;

    buffer.write_all(&MAGIC.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&VERSION.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&node_count.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&stop_count.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&[0u8; 3]).map_err(|_| BusError::IoError)?; // Padding for 4-byte alignment
    buffer.write_all(&grid.x0_cm.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&grid.y0_cm.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&lat_avg_deg.to_le_bytes()).map_err(|_| BusError::IoError)?; // Average latitude for projection

    for node in route_nodes {
        let bytes = unsafe {
            core::slice::from_raw_parts(node as *const RouteNode as *const u8, size_of::<RouteNode>())
        };
        buffer.write_all(bytes).map_err(|_| BusError::IoError)?;
    }

    for stop in stops {
        let bytes = unsafe {
            core::slice::from_raw_parts(stop as *const Stop as *const u8, size_of::<Stop>())
        };
        buffer.write_all(bytes).map_err(|_| BusError::IoError)?;
    }

    buffer.write_all(&grid.cols.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&grid.rows.to_le_bytes()).map_err(|_| BusError::IoError)?;
    buffer.write_all(&grid.grid_size_cm.to_le_bytes()).map_err(|_| BusError::IoError)?;

    let mut index_data = Vec::new();
    let mut offsets = Vec::with_capacity((grid.cols * grid.rows) as usize);
    
    for cell in &grid.cells {
        offsets.push(index_data.len() as u32);
        let count = (cell.len().min(65535)) as u16;
        index_data.write_all(&count.to_le_bytes()).map_err(|_| BusError::IoError)?;
        for &seg_idx in cell {
            index_data.write_all(&(seg_idx as u16).to_le_bytes()).map_err(|_| BusError::IoError)?;
        }
    }

    for offset in offsets {
        buffer.write_all(&offset.to_le_bytes()).map_err(|_| BusError::IoError)?;
    }
    buffer.write_all(&index_data).map_err(|_| BusError::IoError)?;

    if gaussian_lut.len() != 256 || logistic_lut.len() != 128 {
        return Err(BusError::InvalidLength);
    }
    buffer.write_all(gaussian_lut).map_err(|_| BusError::IoError)?;
    buffer.write_all(logistic_lut).map_err(|_| BusError::IoError)?;

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&buffer);
    let crc = hasher.finalize();
    buffer.write_all(&crc.to_le_bytes()).map_err(|_| BusError::IoError)?;

    output.write_all(&buffer).map_err(|_| BusError::IoError)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_and_load() {
        let nodes = vec![];
        let stops = vec![];
        let grid = SpatialGrid {
            cells: vec![],
            grid_size_cm: 10000,
            cols: 0,
            rows: 0,
            x0_cm: 100,
            y0_cm: 200,
        };
        let mut buffer = Vec::new();
        let lat_avg_deg = 25.0;
        pack_route_data(&nodes, &stops, &grid, lat_avg_deg, &[0u8; 256], &[0u8; 128], &mut buffer).unwrap();

        let loaded = RouteData::load(&buffer).unwrap();
        assert_eq!(loaded.x0_cm, 100);
        assert_eq!(loaded.y0_cm, 200);
        assert_eq!(loaded.lat_avg_deg, 25.0);
        assert_eq!(loaded.node_count, 0);
    }

    #[test]
    fn test_crc_mismatch() {
        let mut buffer = Vec::new();
        let grid = SpatialGrid {
            cells: vec![],
            grid_size_cm: 10000,
            cols: 0,
            rows: 0,
            x0_cm: 0,
            y0_cm: 0,
        };
        pack_route_data(&[], &[], &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer).unwrap();

        // Corrupt one byte of data (not the CRC itself)
        buffer[10] ^= 0xFF;

        let result = RouteData::load(&buffer);
        assert_eq!(result.err(), Some(BusError::ChecksumMismatch));
    }

    #[test]
    fn test_invalid_magic() {
        let buffer = vec![0u8; 100];
        let result = RouteData::load(&buffer);
        assert_eq!(result.err(), Some(BusError::InvalidMagic));
    }
}


