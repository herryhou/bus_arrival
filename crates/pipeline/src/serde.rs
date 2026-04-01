//! Unified serde interface for std and no_std

use crate::PipelineError;

/// Serialize a value to JSON string
///
/// Returns the length of the JSON data written to the buffer.
/// Works with both serde_json (std) and serde_json_core (no_std).
///
/// # Arguments
///
/// * `buf` - Buffer to write JSON data to
/// * `value` - Value to serialize
///
/// # Returns
///
/// Returns the number of bytes written to the buffer.
///
/// # Errors
///
/// Returns `PipelineError::SerializationError` if serialization fails.
/// Returns `PipelineError::BufferTooSmall` if the buffer is too small.
pub fn to_string<T: serde::Serialize>(
    buf: &mut [u8],
    value: &T,
) -> Result<usize, PipelineError> {
    #[cfg(feature = "std")]
    {
        let s = serde_json::to_string(value)
            .map_err(|e| PipelineError::SerializationError(e.to_string()))?;
        if s.len() > buf.len() {
            return Err(PipelineError::BufferTooSmall);
        }
        buf[..s.len()].copy_from_slice(s.as_bytes());
        Ok(s.len())
    }
    #[cfg(not(feature = "std"))]
    {
        // Use a fixed-size heapless string for serialization
        // The size must be large enough for typical JSON outputs
        const MAX_JSON_SIZE: usize = 1024;
        let heapless_str: serde_json_core::heapless::String<MAX_JSON_SIZE> = serde_json_core::to_string(value)
            .map_err(|e| PipelineError::SerializationError(format!("{:?}", e)))?;
        let len = heapless_str.len();
        if len > buf.len() {
            return Err(PipelineError::BufferTooSmall);
        }
        buf[..len].copy_from_slice(heapless_str.as_bytes());
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize)]
    struct TestStruct {
        name: &'static str,
        value: i32,
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_to_string_std() {
        let test = TestStruct { name: "test", value: 42 };
        let mut buf = [0u8; 100];
        let len = to_string(&mut buf, &test).unwrap();
        let json = std::str::from_utf8(&buf[..len]).unwrap();
        // Expected: {"name":"test","value":42} (20 bytes)
        // serde_json might add whitespace, so just check it contains the expected content
        assert!(json.contains(r#""name":"test""#));
        assert!(json.contains(r#""value":42"#));
        assert!(json.starts_with('{') && json.ends_with('}'));
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_to_string_buffer_too_small() {
        let test = TestStruct { name: "test", value: 42 };
        let mut buf = [0u8; 10];
        let result = to_string(&mut buf, &test);
        assert!(matches!(result, Err(PipelineError::BufferTooSmall)));
    }
}
