//! Debug test to verify u32 wrap behavior
#![cfg(feature = "std")]

#[test]
fn test_u32_wrap_behavior() {
    // Test what happens when we do: ((50000 - 100000) / 10000) as u32
    let gps_x = 50000i32;
    let x0_cm = 100000i32;
    let grid_size_cm = 10000i32;

    // This is what the current code does
    let diff = gps_x - x0_cm; // -50000
    println!("diff = {}", diff);

    let divided = diff / grid_size_cm; // -5
    println!("divided = {}", divided);

    let wrapped = divided as u32; // This will wrap!
    println!("wrapped as u32 = {}", wrapped);

    // When we cast back to i32
    let back_to_i32 = wrapped as i32;
    println!("back to i32 = {}", back_to_i32);

    // Verify the wrap happens
    assert_eq!(diff, -50000);
    assert_eq!(divided, -5);
    assert!(
        wrapped > 4000000000,
        "Should wrap to ~4 billion, got {}",
        wrapped
    );
    assert_eq!(back_to_i32, -5, "Casting back to i32 should give us -5");

    println!("Confirmed: u32 wrap happens and casting back to i32 restores the negative value");
}

#[test]
fn test_grid_lookup_with_wrapped_coordinates() {
    // When grid lookup is done with wrapped coordinates
    let gx = 4294967291u32; // -5 as i32 (wrapped)
    let gy = 5u32;

    // The current code does:
    let ny = gy as i32 + 0i32 - 1; // 4
    let nx = gx as i32 + 0i32 - 1; // -6

    println!("ny = {}", ny);
    println!("nx = {}", nx);

    // The check `if ny < 0 || nx < 0` will catch this
    assert!(nx < 0, "nx should be negative");
    assert!(ny >= 0, "ny should be non-negative");

    println!("Confirmed: wrapped coordinates produce negative i32 values that are caught by the bounds check");
}
