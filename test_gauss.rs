fn main() {
    let sigma = 2750.0;
    for i in [0, 128, 254, 255].iter() {
        let x = (*i as f64) * 100.0;
        let g = (-0.5 * (x / sigma).powi(2)).exp();
        let val = (g * 255.0).min(255.0) as u8;
        println!("i={}, x={}cm, g={}, val={}", i, x, g, val);
    }
}
