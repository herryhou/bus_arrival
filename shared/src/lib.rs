//! Shared types and binary loader for GPS bus arrival detection system.
//!
//! Optimized for RP2350 (Pico 2) with zero-copy parsing and integer-only logic.

use core::marker::PhantomData;
use core::mem::size_of;

/// Magic bytes for route_data.bin: "BUSA" (BUS Arrival)
pub const MAGIC: u32 = 0x42555341;

/// Format version
pub const VERSION: u16 = 1;

pub type DistCm = i32;
pub type SpeedCms = i32;
pub type HeadCdeg = i16;
pub type Prob8 = u8;
pub type Dist2 = i64;

/// Error types for the bus arrival system
#[derive(Debug, PartialEq)]
pub enum BusError {
    InvalidMagic,
    InvalidVersion,
    InvalidLength,
    ChecksumMismatch,
    OutOfBounds,
}

/// Route node representing a point in a bus route.
/// Layout (52 bytes): len2(8), line_c(8), x(4), y(4), cum_dist(4), dx(4), dy(4), seg_len(4), line_a(4), line_b(4), heading(2), pad(2)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RouteNode {
    pub len2_cm2: i64,
    pub line_c: i64,
    pub x_cm: i32,
    pub y_cm: i32,
    pub cum_dist_cm: i32,
    pub dx_cm: i32,
    pub dy_cm: i32,
    pub seg_len_cm: i32,
    pub line_a: i32,
    pub line_b: i32,
    pub heading_cdeg: i16,
    pub _pad: i16,
}

/// Bus stop with corridor boundaries for arrival detection.
/// Layout (12 bytes): progress(4), start(4), end(4)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stop {
    pub progress_cm: DistCm,
    pub corridor_start_cm: DistCm,
    pub corridor_end_cm: DistCm,
}

/// Spatial grid for O(k) map matching (used by preprocessor to build).
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,
    pub grid_size_cm: DistCm,
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: DistCm,
    pub y0_cm: DistCm,
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
        
        // Read offset (u32) using unaligned read
        let offset_ptr = unsafe { self.offsets_base.add(idx * 4) as *const u32 };
        let start_offset = unsafe { core::ptr::read_unaligned(offset_ptr) } as usize;
        
        // Read count (u8)
        let cell_ptr = unsafe { self.data_base.add(start_offset) };
        let count = unsafe { *cell_ptr } as usize;
        
        // Read segment indices (u16 list)
        let indices_ptr = unsafe { cell_ptr.add(1) as *const u16 };
        
        Ok(unsafe { core::slice::from_raw_parts(indices_ptr, count) })
    }
}

/// The complete route data, referenced directly from a byte slice.
pub struct RouteData<'a> {
    pub x0_cm: i32,
    pub y0_cm: i32,
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
        if data.len() < 17 + 4 {
            return Err(BusError::InvalidLength);
        }

        let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let version = u16::from_le_bytes(data[4..6].try_into().unwrap());
        if magic != MAGIC { return Err(BusError::InvalidMagic); }
        if version != VERSION { return Err(BusError::InvalidVersion); }

        let node_count = u16::from_le_bytes(data[6..8].try_into().unwrap()) as usize;
        let stop_count = data[8] as usize;
        let x0_cm = i32::from_le_bytes(data[9..13].try_into().unwrap());
        let y0_cm = i32::from_le_bytes(data[13..17].try_into().unwrap());

        let received_crc = u32::from_le_bytes(data[data.len()-4..].try_into().unwrap());
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&data[..data.len()-4]);
        if hasher.finalize() != received_crc {
            return Err(BusError::ChecksumMismatch);
        }

        let mut offset = 17;

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
