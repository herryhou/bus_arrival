//! Integration tests for recovery module usage in control layer

#[cfg(test)]
mod recovery_integration_tests {
    use crate::recovery::RecoveryInput;
    use heapless::Vec;
    use shared::Stop;

    #[test]
    fn test_recovery_input_gps_jump() {
        // Test that RecoveryInput can be constructed for GPS jump scenario
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ]).unwrap();

        let input = RecoveryInput {
            s_cm: 5100,
            v_cms: 1000,
            dt_seconds: 5,
            stops,
            hint_idx: 1,
            frozen_s_cm: None,  // GPS jump: no frozen position
            search_window: 10,
        };

        // Should recover to stop 1
        let result = crate::recovery::recover(input);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_recovery_input_reacquisition() {
        // Test that RecoveryInput can be constructed for re-acquisition scenario
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
        ]).unwrap();

        let input = RecoveryInput {
            s_cm: 4800,
            v_cms: 1000,
            dt_seconds: 10,
            stops,
            hint_idx: 1,
            frozen_s_cm: Some(5000),  // Re-acquisition: has frozen position
            search_window: 10,
        };

        // Should recover to stop 1 (within reach, spatial anchor penalty applied)
        let result = crate::recovery::recover(input);
        assert_eq!(result, Some(1));
    }
}
