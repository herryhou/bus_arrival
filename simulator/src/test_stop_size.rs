#[test]
fn test_stop_size() {
    use shared::Stop;
    println!("Rust sizeof(Stop): {}", std::mem::size_of::<Stop>());
    println!("Rust align_of(Stop): {}", std::mem::align_of::<Stop>());
}
