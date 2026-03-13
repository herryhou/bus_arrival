fn main() {
    for i in 0..128 {
        let dv = (i as f64) * 10.0;
        let l = 1.0 / (1.0 + (-0.01 * (dv - 200.0)).exp());
        let val = (l * 255.0).min(255.0) as u8;
        if val > 200 {
            println!("i={}, dv={}, val={}", i, dv, val);
        }
    }
}
