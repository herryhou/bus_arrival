fn main() {
    let i = 40;
    let dv = (i as f64) * 10.0;
    let l = 1.0 / (1.0 + (-0.01 * (dv - 200.0)).exp());
    let val = (l * 255.0).min(255.0) as u8;
    println!("i={}, dv={}, l={}, val={}", i, dv, l, val);
}
