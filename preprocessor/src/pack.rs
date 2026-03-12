// Binary packing: serialize route data to route_data.bin format.
//
// New v8 format:
// [4B] magic (0x42555341)
// [2B] version (1)
// [2B] node_count
// [1B] stop_count
// [4B] x0_cm (grid origin)
// [4B] y0_cm (grid origin)
// [N×52B] route_nodes
// [M×12B] stops
// [var] grid_index
// [256B] gaussian_lut
// [128B] logistic_lut
// [4B] crc32

use shared::{RouteNode, Stop, SpatialGrid, MAGIC, VERSION};
use std::io::{self, Write};
use crc32fast::Hasher;

/// Pack route data into binary format.
pub fn pack_route_data(
    route_nodes: &[RouteNode],
    stops: &[Stop],
    grid: &SpatialGrid,
    gaussian_lut: &[u8],
    logistic_lut: &[u8],
    output: &mut impl Write,
) -> io::Result<()> {
    let mut buffer = Vec::new();
    let node_count = route_nodes.len() as u16;
    let stop_count = stops.len() as u8;

    // 1. Write header to buffer
    buffer.write_all(&MAGIC.to_le_bytes())?;
    buffer.write_all(&VERSION.to_le_bytes())?;
    buffer.write_all(&node_count.to_le_bytes())?;
    buffer.write_all(&stop_count.to_le_bytes())?;
    buffer.write_all(&grid.x0_cm.to_le_bytes())?;
    buffer.write_all(&grid.y0_cm.to_le_bytes())?;

    // 2. Write route nodes (raw bytes, RouteNode is repr(C, packed))
    for node in route_nodes {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                node as *const RouteNode as *const u8,
                std::mem::size_of::<RouteNode>(),
            )
        };
        buffer.write_all(bytes)?;
    }

    // 3. Write stops (raw bytes, Stop is repr(C))
    for stop in stops {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                stop as *const Stop as *const u8,
                std::mem::size_of::<Stop>(),
            )
        };
        buffer.write_all(bytes)?;
    }

    // 4. Write grid index
    // Format: [4B] cols, [4B] rows, [4B] grid_size_cm, 
    //         followed by [4B] offsets table (cols * rows * u32)
    //         followed by [var] segment indices list (u16 per segment index)
    buffer.write_all(&grid.cols.to_le_bytes())?;
    buffer.write_all(&grid.rows.to_le_bytes())?;
    buffer.write_all(&grid.grid_size_cm.to_le_bytes())?;

    let mut index_data = Vec::new();
    let mut offsets = Vec::with_capacity((grid.cols * grid.rows) as usize);
    
    for cell in &grid.cells {
        offsets.push(index_data.len() as u32);
        // Each cell contains a list of segment indices (u16)
        // Store as: [u8] count, [u16...] indices
        let count = (cell.len().min(255)) as u8;
        index_data.push(count);
        for &seg_idx in cell {
            index_data.write_all(&(seg_idx as u16).to_le_bytes())?;
        }
    }

    // Write offsets table
    for offset in offsets {
        buffer.write_all(&offset.to_le_bytes())?;
    }
    // Write actual index data
    buffer.write_all(&index_data)?;

    // 5. Write LUTs
    if gaussian_lut.len() != 256 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Gaussian LUT must be 256 bytes"));
    }
    if logistic_lut.len() != 128 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Logistic LUT must be 128 bytes"));
    }
    buffer.write_all(gaussian_lut)?;
    buffer.write_all(logistic_lut)?;

    // 6. Calculate CRC32 (everything before CRC)
    let mut hasher = Hasher::new();
    hasher.update(&buffer);
    let crc = hasher.finalize();

    // 7. Write CRC and flush to final output
    buffer.write_all(&crc.to_le_bytes())?;
    output.write_all(&buffer)?;

    Ok(())
}
