// Douglas-Peucker polyline simplification algorithm
//
// Implements the Ramer-Douglas-Peucker algorithm for reducing the number of points
// in a polyline while preserving its overall geometry.
//
// v8 Spec Enhancements:
// - ε_general = 700 cm (default tolerance)
// - ε_curve = 250 cm (for turns > 20°)
// - Stop protection radius = ±3000 cm
// - Max segment length = 10000 cm (100m, default for general segments)
// - Adaptive segment length = 3000 cm (30m, near stops/sharp turns)

use std::collections::HashSet;

/// Adaptive segment length for critical areas in cm (30m = 3000cm)
/// Used near stops and sharp turns for precise arrival detection
const ADAPTIVE_SEGMENT_LENGTH_CM: f64 = 3000.0;

/// Simplify a polyline and ensure max segment length by interpolating synthetic points if needed.
pub fn simplify_and_interpolate(
    points: &[(i64, i64)],
    epsilon_cm: f64,
    stop_indices: &[usize],
) -> Vec<(i64, i64)> {
    if points.is_empty() {
        return vec![];
    }

    if points.len() <= 2 {
        let mut result = vec![points[0]];
        if points.len() == 2 {
            interpolate_recursive(points[0], points[1], &mut result, 10000.0);
            result.push(points[1]);
        }
        return result;
    }

    let mut keep = HashSet::new();
    
    // Always keep start and end
    keep.insert(0);
    keep.insert(points.len() - 1);

    // Identify ALL indices that MUST be kept
    let mut split_points = Vec::new();
    split_points.push(0);
    for &idx in stop_indices {
        if idx > 0 && idx < points.len() - 1 {
            split_points.push(idx);
            keep.insert(idx);
        }
    }
    split_points.push(points.len() - 1);
    split_points.sort_unstable();
    split_points.dedup();

    // Step 1: Douglas-Peucker on each interval between split points
    for i in 0..split_points.len() - 1 {
        let start = split_points[i];
        let end = split_points[i+1];
        douglas_peucker_recursive(points, start, end, epsilon_cm, &mut keep);
    }

    let mut kept_indices: Vec<usize> = keep.into_iter().collect();
    kept_indices.sort_unstable();

    // Step 2: Build final point list with interpolation
    let mut final_points = Vec::new();
    for i in 0..kept_indices.len() - 1 {
        let p1 = points[kept_indices[i]];
        let p2 = points[kept_indices[i+1]];
        final_points.push(p1);
        interpolate_recursive(p1, p2, &mut final_points, 10000.0);
    }
    final_points.push(points[*kept_indices.last().unwrap()]);

    final_points
}

/// Recursively subdivide a segment using geometric midpoint insertion
///
/// # Arguments
/// * `p1` - Start point of segment (x_cm, y_cm)
/// * `p2` - End point of segment (x_cm, y_cm)
/// * `result` - Vector to accumulate intermediate points
/// * `max_len` - Maximum segment length in centimeters (e.g., 10000.0 = 100m)
///
/// # Algorithm
/// Uses geometric midpoint splitting: if segment length > max_len,
/// recursively splits at midpoint and adds it to result.
fn interpolate_recursive(p1: (i64, i64), p2: (i64, i64), result: &mut Vec<(i64, i64)>, max_len: f64) {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    let dist = ((dx * dx + dy * dy) as f64).sqrt();

    if dist > max_len {
        // Geometric midpoint
        let mid = (
            (p1.0 + p2.0) / 2,
            (p1.1 + p2.1) / 2,
        );
        
        // Recursive split first half
        interpolate_recursive(p1, mid, result, max_len);
        result.push(mid);
        // Recursive split second half
        interpolate_recursive(mid, p2, result, max_len);
    }
}

fn douglas_peucker_recursive(
    points: &[(i64, i64)],
    start_idx: usize,
    end_idx: usize,
    epsilon_cm: f64,
    keep: &mut HashSet<usize>,
) {
    if end_idx <= start_idx + 1 {
        return;
    }

    let mut furthest_idx = start_idx + 1;
    let mut max_dist = -1.0;
    for i in (start_idx + 1)..end_idx {
        let dist = perpendicular_distance(points[i], points[start_idx], points[end_idx]);
        if dist > max_dist {
            max_dist = dist;
            furthest_idx = i;
        }
    }

    let mut effective_epsilon = epsilon_cm;
    if is_sharp_turn(points, start_idx, furthest_idx, end_idx) {
        effective_epsilon = 250.0; // ε_curve
    }

    if max_dist > effective_epsilon {
        keep.insert(furthest_idx);
        douglas_peucker_recursive(points, start_idx, furthest_idx, epsilon_cm, keep);
        douglas_peucker_recursive(points, furthest_idx, end_idx, epsilon_cm, keep);
    }
}

fn perpendicular_distance(p: (i64, i64), a: (i64, i64), b: (i64, i64)) -> f64 {
    let x0 = p.0 as f64;
    let y0 = p.1 as f64;
    let x1 = a.0 as f64;
    let y1 = a.1 as f64;
    let x2 = b.0 as f64;
    let y2 = b.1 as f64;

    let dx = x2 - x1;
    let dy = y2 - y1;
    let denominator = (dx.powi(2) + dy.powi(2)).sqrt();

    if denominator < 1e-6 {
        return (((x0 - x1).powi(2) + (y0 - y1).powi(2)) as f64).sqrt();
    }

    let numerator = (dy * x0 - dx * y0 + x2 * y1 - y2 * x1).abs();
    numerator / denominator
}

fn is_sharp_turn(points: &[(i64, i64)], a_idx: usize, m_idx: usize, b_idx: usize) -> bool {
    let a = points[a_idx];
    let m = points[m_idx];
    let b = points[b_idx];

    let v1 = (m.0 - a.0, m.1 - a.1);
    let v2 = (b.0 - m.0, b.1 - m.1);

    let dot = v1.0 * v2.0 + v1.1 * v2.1;
    let mag1 = ((v1.0 * v1.0 + v1.1 * v1.1) as f64).sqrt();
    let mag2 = ((v2.0 * v2.0 + v2.1 * v2.1) as f64).sqrt();

    if mag1 < 1.0 || mag2 < 1.0 {
        return false;
    }

    let cos_theta = dot as f64 / (mag1 * mag2);
    let theta = cos_theta.clamp(-1.0, 1.0).acos().to_degrees();

    theta > 20.0
}

/// Calculate Euclidean distance between two points in centimeters
fn distance(p1: (i64, i64), p2: (i64, i64)) -> f64 {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    ((dx * dx + dy * dy) as f64).sqrt()
}

/// Check if a line segment passes within a given distance of a point
/// Returns true if the minimum distance from point to segment is <= threshold
fn segment_near_point(p1: (i64, i64), p2: (i64, i64), point: (i64, i64), threshold_cm: f64) -> bool {
    let px = point.0 as f64;
    let py = point.1 as f64;
    let x1 = p1.0 as f64;
    let y1 = p1.1 as f64;
    let x2 = p2.0 as f64;
    let y2 = p2.1 as f64;

    // Vector from p1 to p2
    let dx = x2 - x1;
    let dy = y2 - y1;

    // Vector from p1 to point
    let ldx = px - x1;
    let ldy = py - y1;

    // Project point onto line, clamped to segment [0, 1]
    let seg_len2 = dx * dx + dy * dy;

    let t = if seg_len2 < 1e-10 {
        // Segment is essentially a point
        0.0
    } else {
        ((ldx * dx + ldy * dy) / seg_len2).clamp(0.0, 1.0)
    };

    // Find closest point on segment
    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;

    // Distance from point to closest point on segment
    let dist_sq = (px - closest_x).powi(2) + (py - closest_y).powi(2);
    dist_sq.sqrt() <= threshold_cm
}

/// Determine if a segment needs refinement based on stop proximity and sharp turns
fn should_refine_segment(
    p1: (i64, i64),
    p2: (i64, i64),
    route: &[(i64, i64)],
    stop_indices: &[usize],
    kept_indices: &[usize],
) -> bool {
    // Check proximity to stops
    for &stop_idx in stop_indices {
        if stop_idx < route.len() {
            let stop = route[stop_idx];
            if segment_near_point(p1, p2, stop, 10000.0) {
                return true;
            }
        }
    }

    // Check for sharp turn at p1
    if let Some(p1_route_idx) = route.iter().position(|&p| p == p1) {
        if let Some(&prev_idx) = kept_indices.iter().rev().find(|&&i| i < p1_route_idx) {
            if prev_idx > 0 && prev_idx < route.len() - 1 {
                // Find b_idx (index of p2)
                if let Some(p2_route_idx) = route.iter().position(|&p| p == p2) {
                    if is_sharp_turn(route, prev_idx, p1_route_idx, p2_route_idx) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Apply adaptive segmentation to a route
///
/// Segments within 100m of stops or at sharp turns are refined to 30m max length.
/// Other segments can be up to 100m in length.
fn adaptive_segmentation(
    route: &[(i64, i64)],
    stop_indices: &[usize],
    kept_indices: &[usize],
) -> Vec<(i64, i64)> {
    if route.len() <= 2 {
        return route.to_vec();
    }

    let mut result = Vec::new();

    for i in 0..route.len() - 1 {
        let p1 = route[i];
        let p2 = route[i + 1];

        let segment_len = distance(p1, p2);
        let needs_refinement = should_refine_segment(p1, p2, route, stop_indices, kept_indices);

        if segment_len > ADAPTIVE_SEGMENT_LENGTH_CM && needs_refinement {
            // Refine to 30m max for critical areas - use existing interpolate_recursive
            result.push(p1);
            interpolate_recursive(p1, p2, &mut result, ADAPTIVE_SEGMENT_LENGTH_CM);
        } else {
            // No refinement needed, just push start point
            result.push(p1);
        }
    }

    result.push(route[route.len() - 1]);
    result
}
