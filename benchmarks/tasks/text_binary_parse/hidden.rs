include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Rejected(TextError::Empty),
        "case 0"
    );
    assert_eq!(solve("0".to_string()), TextOutcome::Accepted(0), "case 1");
    assert_eq!(solve("1".to_string()), TextOutcome::Accepted(1), "case 2");
    assert_eq!(
        solve("00101".to_string()),
        TextOutcome::Accepted(5),
        "case 3"
    );
    assert_eq!(
        solve("1111111111111111".to_string()),
        TextOutcome::Accepted(65535),
        "case 4"
    );
    assert_eq!(
        solve("1000000000000000".to_string()),
        TextOutcome::Accepted(32768),
        "case 5"
    );
    assert_eq!(
        solve("00000000000000000".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 6"
    );
    assert_eq!(
        solve("2".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve("10x".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 8"
    );
    assert_eq!(
        solve(" 1".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 9"
    );
    assert_eq!(
        solve("+1".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 10"
    );
    assert_eq!(
        solve("101010".to_string()),
        TextOutcome::Accepted(42),
        "case 11"
    );
    assert_eq!(
        solve("xxxxxxxxxxxxxxxxx".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 12"
    );
}
