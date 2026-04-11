//! Test speed constraint matches spec Section 9.1
//! D_max = V_max * 1s + sigma_gps = 1667 + 2000 = 3667 cm

use gps_processor::kalman::check_speed_constraint;

#[test]
fn test_speed_constraint_rejects_37m_jump() {
    // Position change of 37 m = 3700 cm exceeds D_max = 3667 cm
    let z_new = 10000 + 3700;
    let z_prev = 10000;
    let dt = 1;

    assert!(!check_speed_constraint(z_new, z_prev, dt));
}

#[test]
fn test_speed_constraint_allows_36m_jump() {
    // Position change of 36 m = 3600 cm within D_max = 3667 cm
    let z_new = 10000 + 3600;
    let z_prev = 10000;
    let dt = 1;

    assert!(check_speed_constraint(z_new, z_prev, dt));
}

#[test]
fn test_speed_constraint_dt_scaling() {
    // With dt=2, D_max = 1667*2 + 2000 = 5334 cm
    let z_new = 10000 + 5300;  // 53 m
    let z_prev = 10000;
    let dt = 2;

    assert!(check_speed_constraint(z_new, z_prev, dt));
}

#[test]
fn test_speed_constraint_current_value_too_permissive() {
    // Current implementation allows 80 m (8000 cm) - this should fail
    // After fix, 80 m should be rejected
    let z_new = 10000 + 8000;
    let z_prev = 10000;
    let dt = 1;

    // This will PASS with current code (wrong), FAIL after fix (correct)
    assert!(!check_speed_constraint(z_new, z_prev, dt),
        "80 m jump should be rejected but currently passes");
}
