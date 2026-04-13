//! PersistedState CRC validation tests (host-only)
//!
//! These tests verify the CRC32 round-trip logic without requiring
//! actual flash hardware. The firmware uses the same code path.

use shared::PersistedState;

#[test]
fn test_persisted_state_crc_roundtrip() {
    let state = PersistedState::new(123_456, 7);
    assert!(state.is_valid(), "Freshly created state should have valid CRC");
    assert_eq!(state.last_progress_cm, 123_456);
    assert_eq!(state.last_stop_index, 7);
}

#[test]
fn test_persisted_state_corruption_detected() {
    let mut state = PersistedState::new(123_456, 7);
    assert!(state.is_valid());

    // Corrupt one byte
    state.last_stop_index = 8;
    assert!(!state.is_valid(), "Corrupted state should fail CRC check");
}

#[test]
fn test_persisted_state_size() {
    // Critical: flash read/write uses raw bytes
    assert_eq!(core::mem::size_of::<PersistedState>(), 12);
}

#[test]
fn test_persisted_state_invalid_sentinel() {
    assert_eq!(PersistedState::INVALID.last_progress_cm, 0);
    assert_eq!(PersistedState::INVALID.last_stop_index, 0);
    assert!(!PersistedState::INVALID.is_valid(), "INVALID should fail CRC");
}

#[test]
fn test_persisted_state_negative_progress() {
    // Negative progress is valid during cold-start before Kalman converges
    let state = PersistedState::new(-1000, 0);
    assert!(state.is_valid());
    assert_eq!(state.last_progress_cm, -1000);
}

#[test]
fn test_persisted_state_max_stop_index() {
    // Test with maximum plausible stop index (255)
    let state = PersistedState::new(1_000_000, 255);
    assert!(state.is_valid());
    assert_eq!(state.last_stop_index, 255);
}
