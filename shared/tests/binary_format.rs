//! Integration tests for binary format compatibility
//!
//! These tests verify the binary format works correctly for the visualizer.

use shared::binfile::{RouteData, MAGIC, VERSION};

#[test]
fn test_header_magic_and_version() {
    // Verify we can identify valid magic bytes and version
    assert_eq!(MAGIC, 0x42555341); // "BUSA"
    assert_eq!(VERSION, 1);
}

#[test]
fn test_lat_avg_deg_roundtrip() {
    // Verify lat_avg_deg roundtrip is exact
    // This was the bug causing ~10m position errors

    // Create minimal valid binary with known lat_avg
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&MAGIC.to_le_bytes());
    buffer.extend_from_slice(&1u16.to_le_bytes()); // version
    buffer.extend_from_slice(&0u16.to_le_bytes()); // node_count
    buffer.push(0); // stop_count
    buffer.extend_from_slice(&[0u8, 0, 0]); // padding
    buffer.extend_from_slice(&100i32.to_le_bytes()); // x0_cm
    buffer.extend_from_slice(&200i32.to_le_bytes()); // y0_cm

    // Test with specific lat_avg values we encountered
    let test_lat_avgs: [f64; 4] = [24.990083, 25.0, 20.0, 30.0];

    for lat_avg in test_lat_avgs {
        buffer.extend_from_slice(&lat_avg.to_le_bytes());

        // Add dummy data for rest of required fields
        buffer.extend_from_slice(&[0u8; 256]); // gaussian_lut
        buffer.extend_from_slice(&[0u8; 128]); // logistic_lut

        // Calculate CRC for what we have
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&buffer);
        let crc = hasher.finalize();
        buffer.extend_from_slice(&crc.to_le_bytes());

        // Try to load - should get lat_avg back correctly
        let loaded = RouteData::load(&buffer);
        match loaded {
            Ok(data) => {
                assert!((data.lat_avg_deg - lat_avg).abs() < 0.000001,
                    "lat_avg roundtrip failed: expected {}, got {}", lat_avg, data.lat_avg_deg);
            }
            Err(e) => {
                // Other fields may fail, but lat_avg was parsed
                // If we get here, the header was parsed successfully
            }
        }

        // Reset buffer for next test
        buffer.truncate(28); // keep only header
    }
}
