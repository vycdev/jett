include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Accepted("".to_string()),
        "case 0"
    );
    assert_eq!(
        solve("41".to_string()),
        TextOutcome::Accepted("A".to_string()),
        "case 1"
    );
    assert_eq!(
        solve("6869".to_string()),
        TextOutcome::Accepted("hi".to_string()),
        "case 2"
    );
    assert_eq!(
        solve("207E".to_string()),
        TextOutcome::Accepted(" ~".to_string()),
        "case 3"
    );
    assert_eq!(
        solve("5c22".to_string()),
        TextOutcome::Accepted("\\\"".to_string()),
        "case 4"
    );
    assert_eq!(
        solve("0".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 5"
    );
    assert_eq!(
        solve("GG".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 6"
    );
    assert_eq!(
        solve("00".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 7"
    );
    assert_eq!(
        solve("7f".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 8"
    );
    assert_eq!(
        solve("ff".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 9"
    );
    assert_eq!(
        solve("00g".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 10"
    );
    assert_eq!(
        solve("002G".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 11"
    );
    assert_eq!(
        solve("2G00".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 12"
    );
}
