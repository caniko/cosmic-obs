use proptest::prelude::*;
use screencast_portal_mock::{RestoreAction, SourceTypes};

proptest! {
    #[test]
    fn restore_action_round_trip(v in 0u32..=2) {
        let action = RestoreAction::try_from(v).unwrap();
        prop_assert_eq!(action as u32, v);
    }

    #[test]
    fn restore_action_rejects_invalid(v in 3u32..=u32::MAX) {
        prop_assert!(RestoreAction::try_from(v).is_err());
    }

    #[test]
    fn source_types_from_bits_truncate_never_panics(v: u32) {
        let _ = SourceTypes::from_bits_truncate(v);
    }

    #[test]
    fn source_types_known_bits_round_trip(v in 0u32..=7) {
        let flags = SourceTypes::from_bits_truncate(v);
        prop_assert_eq!(flags.bits(), v);
    }
}
