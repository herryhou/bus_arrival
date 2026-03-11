// Offline preprocessor for GPS bus arrival detection system
//
// This crate handles:
// - Route simplification (Douglas-Peucker)
// - Route linearization (geometric coefficients)
// - Stop projection onto route
// - Binary data packing

pub mod coord;
pub mod input;
pub mod linearize;
pub mod pack;
pub mod route;
pub mod simplify;
pub mod stops;

pub fn hello() -> &'static str {
    "Preprocessor placeholder"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        assert_eq!(hello(), "Preprocessor placeholder");
    }
}
