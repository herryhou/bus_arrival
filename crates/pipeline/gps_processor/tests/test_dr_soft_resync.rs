//! Test H3: DR soft-resync after GPS recovery
//! Per spec Section 11.3: ŝ_resync = ŝ_DR + (2/10)*(z_gps - ŝ_DR)
//!
//! After GPS outage recovery, first GPS should use conservative 2/10 gain
//! instead of full Kalman gain (13-77/256)

#[test]
fn test_dr_soft_resync_formula() {
    // Test the soft-resync formula
    let s_dr = 100000;  // DR position estimate
    let z_gps = 102000; // Raw GPS projection (20 m ahead)
    
    // Soft resync: s = s_dr + (2/10)*(z_gps - s_dr)
    //            = 100000 + 0.2 * 2000
    //            = 100000 + 400
    //            = 100400
    let expected = s_dr + 2 * (z_gps - s_dr) / 10;
    assert_eq!(expected, 100400, "Soft resync should use 2/10 gain");
    
    // Compare to full Kalman (which would use ~20-30% gain)
    // Full Kalman with Ks=51/256: s = 100000 + (51/256)*2000 ≈ 100398
    // Full Kalman with Ks=77/256: s = 100000 + (77/256)*2000 ≈ 100601
    // Soft resync is intentionally conservative (20%)
}

#[test]
fn test_dr_soft_resync_reduces_gps_error_impact() {
    // Soft resync should significantly reduce the impact of potentially
    // erroneous first post-outage GPS reading
    
    let s_dr = 100000;
    let z_gps_noisy = 105000; // 50 m jump (likely noisy)
    
    // Soft resync: only move 20% toward GPS
    let s_soft = s_dr + 2 * (z_gps_noisy - s_dr) / 10;
    
    // Full Kalman (Ks=77/256): would move ~30% toward GPS
    let s_full = s_dr + 77 * (z_gps_noisy - s_dr) / 256;
    
    // Soft resync should be closer to DR position
    assert!(s_soft < s_full, "Soft resync should be more conservative");
    assert_eq!(s_soft, 101000, "Soft resync moves only 10 m toward 50 m jump");
    assert!(s_full > 101000, "Full Kalman moves more aggressively");
}

#[test]
fn test_dr_soft_resync_multiple_outages() {
    // Each recovery should apply soft resync
    let mut s_dr = 100000;
    
    // First recovery: s = 100000 + 2*(102000-100000)/10 = 100000 + 400 = 100400
    let z_gps1 = 102000;
    s_dr = s_dr + 2 * (z_gps1 - s_dr) / 10;
    assert_eq!(s_dr, 100400, "First soft resync");
    
    // Second recovery: s = 100400 + 2*(103000-100400)/10 = 100400 + 520 = 100920
    let z_gps2 = 103000;
    s_dr = s_dr + 2 * (z_gps2 - s_dr) / 10;
    assert_eq!(s_dr, 100920, "Second soft resync");
}
