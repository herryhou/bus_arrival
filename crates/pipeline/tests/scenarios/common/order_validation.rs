//! Arrival order validation helpers
//!
//! Verifies that stops are detected in the correct order (monotonically increasing).

/// Validate that arrivals are in correct order
pub fn validate_arrival_order(arrivals: &[usize]) -> Result<(), String> {
    let mut last = 0usize;

    for (i, &stop_idx) in arrivals.iter().enumerate() {
        if stop_idx < last {
            return Err(format!(
                "Arrival order violation at position {}: stop {} detected after stop {}",
                i, stop_idx, last
            ));
        }
        last = stop_idx;
    }

    Ok(())
}

/// Validate that arrivals are in correct order and without duplicates
pub fn validate_arrival_order_strict(arrivals: &[usize]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let mut last = 0usize;

    for (i, &stop_idx) in arrivals.iter().enumerate() {
        if stop_idx < last {
            return Err(format!(
                "Arrival order violation at position {}: stop {} detected after stop {}",
                i, stop_idx, last
            ));
        }

        if seen.contains(&stop_idx) {
            return Err(format!(
                "Duplicate arrival at position {}: stop {} already detected",
                i, stop_idx
            ));
        }

        seen.insert(stop_idx);
        last = stop_idx;
    }

    Ok(())
}
