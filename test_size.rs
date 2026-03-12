#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct RouteNode {
    x_cm: i32,
    y_cm: i32,
    dx_cm: i32,
    dy_cm: i32,
    len2_cm2: i64,
    seg_len_cm: i32,
    cum_dist_cm: i32,
    line_a: i32,
    line_b: i32,
    line_c: i32,
    heading_cdeg: i32,
    _pad: i32,
}

fn main() {
    println!("Size of RouteNode: {}", std::mem::size_of::<RouteNode>());
    println!("Align of RouteNode: {}", std::mem::align_of::<RouteNode>());
}
