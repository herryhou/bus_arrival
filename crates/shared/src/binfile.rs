//! Binary file format handling for bus arrival system.//!
//! Defines the MAGIC bytes, versioning, and zero-copy loading logic
//! used by both the preprocessor (for packing) and the runtime (for loading).

use crate::{RouteNode, Stop};
#[cfg(feature = "std")]
use crate::SpatialGrid;
use core::marker::PhantomData;
use core::mem::size_of;
use core::convert::TryInto;
use core::iter::Iterator;
use core::result::Result;
use core::result::Result::{Ok, Err};
use core::option::Option::{self, Some, None};

/// CRC32 lookup table (precomputed for performance)
const CRC32_TABLE: [u32; 256] = [
    0x00000000, 0x77073096, 0xee0e612c, 0x990951ba, 0x076dc419, 0x706af48f,
    0xe963a535, 0x9e6495a3, 0x0edb8832, 0x79dcb8a4, 0xe0d5e91e, 0x97d2d988,
    0x09b64c2b, 0x7eb17cbd, 0xe7b82d07, 0x90bf1d91, 0x1db71064, 0x6ab020f2,
    0xf3b97148, 0x84be41de, 0x1adad47d, 0x6ddde4eb, 0xf4d4b551, 0x83d385c7,
    0x136c9856, 0x646ba8c0, 0xfd62f97a, 0x8a65c9ec, 0x14015c4f, 0x63066cd9,
    0xfa0f3d63, 0x8d080df5, 0x3b6e20c8, 0x4c69105e, 0xd56041e4, 0xa2677172,
    0x3c03e4d1, 0x4b04d447, 0xd20d85fd, 0xa50ab56b, 0x35b5a8fa, 0x42b2986c,
    0xdbbbc9d6, 0xacbcf940, 0x32d86ce3, 0x45df5c75, 0xdcd60dcf, 0xabd13d59,
    0x26d930ac, 0x51de003a, 0xc8d75180, 0xbfd06116, 0x21b4f4b5, 0x56b3c423,
    0xcfba9599, 0xb8bda50f, 0x2802b89e, 0x5f058808, 0xc60cd9b2, 0xb10be924,
    0x2f6f7c87, 0x58684c11, 0xc1611dab, 0xb6662d3d, 0x76dc4190, 0x01db7106,
    0x98d220bc, 0xefd5102a, 0x71b18589, 0x06b6b51f, 0x9fbfe4a5, 0xe8b8d433,
    0x7807c9a2, 0x0f00f934, 0x9609a88e, 0xe10e9818, 0x7f6a0dbb, 0x086d3d2d,
    0x91646c97, 0xe6635c01, 0x6b6b51f4, 0x1c6c6162, 0x856530d8, 0xf262004e,
    0x6c0695ed, 0x1b01a57b, 0x8208f4c1, 0xf50fc457, 0x65b0d9c6, 0x12b7e950,
    0x8bbeb8ea, 0xfcb9887c, 0x62dd1ddf, 0x15da2d49, 0x8cd37cf3, 0xfbd44c65,
    0x4db26158, 0x3ab551ce, 0xa3bc0074, 0xd4bb30e2, 0x4adfa541, 0x3dd895d7,
    0xa4d1c46d, 0xd3d6f4fb, 0x4369e96a, 0x346ed9fc, 0xad678846, 0xda60b8d0,
    0x44042d73, 0x33031de5, 0xaa0a4c5f, 0xdd0d7cc9, 0x5005713c, 0x270241aa,
    0xbe0b1010, 0xc90c2086, 0x5768b525, 0x206f85b3, 0xb966d409, 0xce61e49f,
    0x5edef90e, 0x29d9c998, 0xb0d09822, 0xc7d7a8b4, 0x59b33d17, 0x2eb40d81,
    0xb7bd5c3b, 0xc0ba6cad, 0xedb88320, 0x9abfb3b6, 0x03b6e20c, 0x74b1d29a,
    0xead54739, 0x9dd277af, 0x04db2615, 0x73dc1683, 0xe3630b12, 0x94643b84,
    0x0d6d6a3e, 0x7a6a5aa8, 0xe40ecf0b, 0x9309ff9d, 0x0a00ae27, 0x7d079eb1,
    0xf00f9344, 0x8708a3d2, 0x1e01f268, 0x6906c2fe, 0xf762575d, 0x806567cb,
    0x196c3671, 0x6e6b06e7, 0xfed41b76, 0x89d32be0, 0x10da7a5a, 0x67dd4acc,
    0xf9b9df6f, 0x8ebeeff9, 0x17b7be43, 0x60b08ed5, 0xd6d6a3e8, 0xa1d1937e,
    0x38d8c2c4, 0x4fdff252, 0xd1bb67f1, 0xa6bc5767, 0x3fb506dd, 0x48b2364b,
    0xd80d2bda, 0xaf0a1b4c, 0x36034af6, 0x41047a60, 0xdf60efc3, 0xa867df55,
    0x316e8eef, 0x4669be79, 0xcb61b38c, 0xbc66831a, 0x256fd2a0, 0x5268e236,
    0xcc0c7795, 0xbb0b4703, 0x220216b9, 0x5505262f, 0xc5ba3bbe, 0xb2bd0b28,
    0x2bb45a92, 0x5cb36a04, 0xc2d7ffa7, 0xb5d0cf31, 0x2cd99e8b, 0x5bdeae1d,
    0x9b64c2b0, 0xec63f226, 0x756aa39c, 0x026d930a, 0x9c0906a9, 0xeb0e363f,
    0x72076785, 0x05005713, 0x95bf4a82, 0xe2b87a14, 0x7bb12bae, 0x0cb61b38,
    0x92d28e9b, 0xe5d5be0d, 0x7cdcefb7, 0x0bdbdf21, 0x86d3d2d4, 0xf1d4e242,
    0x68ddb3f8, 0x1fda836e, 0x81be16cd, 0xf6b9265b, 0x6fb077e1, 0x18b74777,
    0x88085ae6, 0xff0f6a70, 0x66063bca, 0x11010b5c, 0x8f659eff, 0xf862ae69,
    0x616bffd3, 0x166ccf45, 0xa00ae278, 0xd70dd2ee, 0x4e048354, 0x3903b3c2,
    0xa7672661, 0xd06016f7, 0x4969474d, 0x3e6e77db, 0xaed16a4a, 0xd9d65adc,
    0x40df0b66, 0x37d83bf0, 0xa9bcae53, 0xdebb9ec5, 0x47b2cf7f, 0x30b5ffe9,
    0xbdbdf21c, 0xcabac28a, 0x53b39330, 0x24b4a3a6, 0xbad03605, 0xcdd70693,
    0x54de5729, 0x23d967bf, 0xb3667a2e, 0xc4614ab8, 0x5d681b02, 0x2a6f2b94,
    0xb40bbe37, 0xc30c8ea1, 0x5a05df1b, 0x2d02ef8d,
];

/// Compute CRC32 checksum (no_std compatible)
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc = CRC32_TABLE[((crc as u8) ^ byte) as usize] ^ (crc >> 8);
    }
    !crc
}

#[cfg(feature = "std")]
use std::io::Write;

/// Pack route data into binary format.
///
/// This function is intended for use in the preprocessor (requires std).

/// Magic bytes for route_data.bin: "BUSA" (BUS Arrival)
pub const MAGIC: u32 = 0x42555341;

/// Format version
/// v2: Removed line_a, line_b, line_c from RouteNode (52 → 36 bytes)
/// v3 (v8.5): Changed repr(C, packed) to repr(C) to fix UB with field references
///             Size now 40 bytes on platforms with 8-byte i64 alignment
/// v4 (v8.7): RouteNode optimization - remove len2_cm2, seg_len_cm→seg_len_mm (i64),
///             dx_cm/dy_cm i32→i16. Size now 32 bytes (28 data + 4 padding).
/// v5 (v8.8): Grid optimization - bitmask for sparse cells + u16 offsets.
///             Grid space reduced from ~16KB to ~5KB (60-70% savings).
pub const VERSION: u16 = 5;

/// Error types for the bus arrival binary file handling
#[cfg_attr(feature = "std", derive(Debug, PartialEq))]
pub enum BusError {
    InvalidMagic,
    InvalidVersion,
    InvalidLength,
    ChecksumMismatch,
    OutOfBounds,
    IoError,
    GridDataOverflow, // Grid data exceeds u16 offset limit (64KB)
}

// Debug implementation for no_std
#[cfg(not(feature = "std"))]
impl core::fmt::Debug for BusError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BusError::InvalidMagic => write!(f, "InvalidMagic"),
            BusError::InvalidVersion => write!(f, "InvalidVersion"),
            BusError::InvalidLength => write!(f, "InvalidLength"),
            BusError::ChecksumMismatch => write!(f, "ChecksumMismatch"),
            BusError::OutOfBounds => write!(f, "OutOfBounds"),
            BusError::IoError => write!(f, "IoError"),
            BusError::GridDataOverflow => write!(f, "GridDataOverflow"),
        }
    }
}

#[cfg(feature = "std")]
impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusError::InvalidMagic => write!(f, "Invalid magic bytes"),
            BusError::InvalidVersion => write!(f, "Invalid version"),
            BusError::InvalidLength => write!(f, "Invalid length"),
            BusError::ChecksumMismatch => write!(f, "Checksum mismatch"),
            BusError::OutOfBounds => write!(f, "Out of bounds"),
            BusError::IoError => write!(f, "I/O error"),
            BusError::GridDataOverflow => write!(f, "Grid data exceeds 64KB limit (u16 offset overflow)"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BusError {}

/// A read-only view into the spatial grid index.
/// Enables O(1) cell access directly from Flash memory.
///
/// v5 format (sparse grid):
/// - Bitmask (1 bit per cell): 1 = cell has data, 0 = empty
/// - Offsets table (u16 per non-empty cell): offset into data section
/// - Data section: count (u16) + segment indices (u16 each)
pub struct SpatialGridView<'a> {
    pub cols: u32,
    pub rows: u32,
    pub grid_size_cm: i32,
    bitmask_base: *const u8,
    offsets_base: *const u8,
    data_base: *const u8,
    _marker: PhantomData<&'a u8>,
}

impl<'a> SpatialGridView<'a> {
    /// Returns the segment indices for a specific cell.
    /// Uses bitmask for sparse cell lookup and u16 offsets.
    pub fn get_cell(&self, col: u32, row: u32) -> Result<&'a [u16], BusError> {
        if col >= self.cols || row >= self.rows {
            return Err(BusError::OutOfBounds);
        }
        let cell_idx = (row * self.cols + col) as usize;

        // Check bitmask to see if cell has data
        let byte_idx = cell_idx / 8;
        let bit_mask = 1 << (cell_idx % 8);
        let bitmask_byte = unsafe { *self.bitmask_base.add(byte_idx) };

        if bitmask_byte & bit_mask == 0 {
            // Empty cell
            return Ok(&[]);
        }

        // Find the offset index by counting set bits before this cell
        // This gives us the index into the offsets table
        let offset_idx = self.count_set_bits_before(cell_idx);

        // Read offset as u16 (2 bytes)
        let offset_ptr = unsafe { self.offsets_base.add(offset_idx * 2) as *const u16 };
        let start_offset = unsafe { core::ptr::read_unaligned(offset_ptr) } as usize;

        // Calculate actual pointer into data section
        let data_ptr = unsafe { self.data_base.add(start_offset) };

        // Read count (first u16 in cell data)
        let count = unsafe { core::ptr::read_unaligned(data_ptr as *const u16) } as usize;

        // Return empty slice if count is 0
        if count == 0 {
            return Ok(&[]);
        }

        // Read segment indices - use unaligned reads
        let indices_ptr = unsafe { data_ptr.add(2) as *const u16 };

        // Check alignment and handle appropriately
        if indices_ptr as usize % 2 == 0 {
            // Aligned, can use from_raw_parts directly
            Ok(unsafe { core::slice::from_raw_parts(indices_ptr, count) })
        } else {
            // Unaligned - need to handle XIP scenario where bin file is at odd address
            #[cfg(feature = "std")]
            {
                // Use unaligned reads for each element into a leaked Vec
                // This is rare but necessary for compatibility
                let vec: Vec<u16> = (0..count)
                    .map(|i| unsafe { core::ptr::read_unaligned(indices_ptr.add(i)) })
                    .collect();

                // Leak the Vec to get a &'static reference, then transmute to &'a [u16]
                // This is safe because:
                // 1. The data is a copy of flash contents (not modifying flash)
                // 2. The leaked Vec lives forever (same as flash lifetime)
                // 3. The 'a lifetime is valid because the original data outlives 'a
                let leaked: &'static [u16] = vec.leak();
                Ok(unsafe { core::mem::transmute::<&'static [u16], &'a [u16]>(leaked) })
            }
            #[cfg(not(feature = "std"))]
            {
                // For no_std, we can't handle unaligned pointers properly
                // Return an error - data should be aligned in firmware
                Err(BusError::InvalidLength)
            }
        }
    }

    /// Count the number of set bits (1s) in the bitmask before the given index.
    /// This is used to find the offset index for a cell.
    #[inline]
    fn count_set_bits_before(&self, idx: usize) -> usize {
        let mut count = 0;
        let mut i = 0;

        // Count full bytes
        while i + 8 <= idx {
            let byte = unsafe { *self.bitmask_base.add(i / 8) };
            count += byte.count_ones() as usize;
            i += 8;
        }

        // Count remaining bits in the partial byte
        let remaining = idx - i;
        if remaining > 0 {
            let byte = unsafe { *self.bitmask_base.add(i / 8) };
            let mask = (1u8 << remaining) - 1;
            count += (byte & mask).count_ones() as usize;
        }

        count
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

    /// Get all stops as a Vec (for iteration/corridor filter computation).
    #[cfg(feature = "std")]
    pub fn stops(&self) -> Vec<Stop> {
        (0..self.stop_count)
            .filter_map(|i| self.get_stop(i))
            .collect()
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
        let computed_crc = crc32(&data[..data.len()-4]);
        if computed_crc != received_crc {
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
        // v5: bitmask (1 bit per cell, rounded up to whole bytes)
        let bitmask_bytes = (cell_count + 7) / 8;
        if data.len() < offset + bitmask_bytes { return Err(BusError::InvalidLength); }
        let bitmask_base = data[offset..].as_ptr();
        offset += bitmask_bytes;

        // Count non-empty cells from bitmask
        let non_empty_count = (0..bitmask_bytes)
            .filter(|&i| unsafe { *bitmask_base.add(i) } != 0)
            .map(|i| unsafe { (*bitmask_base.add(i)).count_ones() as usize })
            .sum::<usize>();

        // v5: u16 offsets (2 bytes per non-empty cell)
        let offsets_size = non_empty_count * 2;
        if data.len() < offset + offsets_size { return Err(BusError::InvalidLength); }
        let offsets_base = data[offset..].as_ptr();
        offset += offsets_size;

        // v5: Skip padding to ensure cell data is 2-byte aligned
        while offset % 2 != 0 {
            offset += 1;
        }

        let grid_data_start = offset;
        if data.len() < grid_data_start { return Err(BusError::InvalidLength); }

        let grid = SpatialGridView {
            cols,
            rows,
            grid_size_cm,
            bitmask_base,
            offsets_base,
            data_base: data[grid_data_start..].as_ptr(),
            _marker: PhantomData,
        };

        Ok(RouteData {
            x0_cm,
            y0_cm,
            lat_avg_deg,
            node_count,
            stop_count,
            nodes_ptr,
            stops_ptr,
            grid,
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

    // v5: Build bitmask and sparse offsets
    let cell_count = (grid.cols * grid.rows) as usize;
    let bitmask_bytes = (cell_count + 7) / 8;
    let mut bitmask = vec![0u8; bitmask_bytes];

    let mut index_data = Vec::new();
    let mut offsets = Vec::new(); // Only for non-empty cells

    for (idx, cell) in grid.cells.iter().enumerate() {
        if !cell.is_empty() {
            // Set bitmask bit
            bitmask[idx / 8] |= 1 << (idx % 8);
            // Store offset (u16) - check for overflow
            let current_offset = index_data.len();
            if current_offset > u16::MAX as usize {
                return Err(BusError::GridDataOverflow);
            }
            offsets.push(current_offset as u16);
            // Write cell data
            let count = (cell.len().min(65535)) as u16;
            index_data.write_all(&count.to_le_bytes()).map_err(|_| BusError::IoError)?;
            for &seg_idx in cell {
                index_data.write_all(&(seg_idx as u16).to_le_bytes()).map_err(|_| BusError::IoError)?;
            }
        }
    }

    // Write bitmask
    buffer.write_all(&bitmask).map_err(|_| BusError::IoError)?;
    // Write u16 offsets (only for non-empty cells)
    for offset in offsets {
        buffer.write_all(&offset.to_le_bytes()).map_err(|_| BusError::IoError)?;
    }
    // Add padding to ensure cell data is 2-byte aligned
    while buffer.len() % 2 != 0 {
        buffer.push(0);
    }
    // Write cell data
    buffer.write_all(&index_data).map_err(|_| BusError::IoError)?;

    let crc = crc32(&buffer);
    buffer.write_all(&crc.to_le_bytes()).map_err(|_| BusError::IoError)?;

    output.write_all(&buffer).map_err(|_| BusError::IoError)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
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
        pack_route_data(&nodes, &stops, &grid, lat_avg_deg, &mut buffer).unwrap();

        let loaded = RouteData::load(&buffer).unwrap();
        assert_eq!(loaded.x0_cm, 100);
        assert_eq!(loaded.y0_cm, 200);
        assert_eq!(loaded.lat_avg_deg, 25.0);
        assert_eq!(loaded.node_count, 0);
    }

    #[cfg(feature = "std")]
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
        pack_route_data(&[], &[], &grid, 25.0, &mut buffer).unwrap();

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

    #[test]
    fn test_grid_misaligned_access() {
        // Test XIP scenario: grid data at odd memory address
        // When bin file is loaded at odd flash address, data section becomes misaligned
        // The fix handles this by copying to a heap-allocated Vec

        // Create grid data with a known pattern
        let mut grid_data = vec![0u8; 16];
        grid_data[0..2].copy_from_slice(&2u16.to_le_bytes()); // count = 2
        grid_data[2..4].copy_from_slice(&42u16.to_le_bytes()); // index[0] = 42
        grid_data[4..6].copy_from_slice(&99u16.to_le_bytes()); // index[1] = 99

        // Try to create a buffer where grid_data starts at odd offset
        let mut misaligned_result: Option<(Vec<u8>, SpatialGridView)> = None;

        for prefix_len in [1usize, 3, 5, 7] {
            let mut buf = vec![0u8; prefix_len];
            buf.extend_from_slice(&grid_data);
            let data_base = buf[prefix_len..].as_ptr();

            if data_base as usize % 2 != 0 {
                misaligned_result = Some((buf, SpatialGridView {
                    cols: 1, rows: 1, grid_size_cm: 10000,
                    bitmask_base: [1u8].as_ptr(),
                    offsets_base: [0u8, 0u8].as_ptr(),
                    data_base, _marker: PhantomData,
                }));
                break;
            }
        }

        let (_buf, misaligned_grid) = match misaligned_result {
            Some(t) => t,
            None => return, // Skip if allocator is too aligned (rare)
        };

        // Should handle misaligned access without panic
        let cell_data = misaligned_grid.get_cell(0, 0).unwrap();
        assert_eq!(cell_data, &[42, 99], "Data should match even when misaligned");
    }
}


