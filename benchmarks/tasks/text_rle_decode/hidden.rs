include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Accepted("".to_string()),
        "case 0"
    );
    assert_eq!(
        solve("1A".to_string()),
        TextOutcome::Accepted("A".to_string()),
        "case 1"
    );
    assert_eq!(
        solve("3A2B".to_string()),
        TextOutcome::Accepted("AAABB".to_string()),
        "case 2"
    );
    assert_eq!(
        solve("1A1A".to_string()),
        TextOutcome::Accepted("AA".to_string()),
        "case 3"
    );
    assert_eq!(solve("100Z".to_string()), TextOutcome::Accepted("ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ".to_string()), "case 4");
    assert_eq!(
        solve("0A".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 5"
    );
    assert_eq!(
        solve("01A".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 6"
    );
    assert_eq!(
        solve("101A".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve("2a".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 8"
    );
    assert_eq!(
        solve("A".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 9"
    );
    assert_eq!(
        solve("12".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 10"
    );
    assert_eq!(
        solve("60A41B".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 11"
    );
    assert_eq!(
        solve("2A0B".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 12"
    );
}
