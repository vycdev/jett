include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Accepted("".to_string()),
        "case 0"
    );
    assert_eq!(
        solve("abc".to_string()),
        TextOutcome::Accepted("abc".to_string()),
        "case 1"
    );
    assert_eq!(
        solve("a+b".to_string()),
        TextOutcome::Accepted("a+b".to_string()),
        "case 2"
    );
    assert_eq!(
        solve("%41%20%7e".to_string()),
        TextOutcome::Accepted("A ~".to_string()),
        "case 3"
    );
    assert_eq!(
        solve("%25".to_string()),
        TextOutcome::Accepted("%".to_string()),
        "case 4"
    );
    assert_eq!(
        solve("%".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 5"
    );
    assert_eq!(
        solve("%2".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 6"
    );
    assert_eq!(
        solve("%GG".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve("%00".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 8"
    );
    assert_eq!(
        solve("%7F".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 9"
    );
    assert_eq!(
        solve("%ff".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 10"
    );
    assert_eq!(
        solve("%00%G0".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 11"
    );
    assert_eq!(
        solve("%G0%00".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 12"
    );
    assert_eq!(
        solve("x%2fy".to_string()),
        TextOutcome::Accepted("x/y".to_string()),
        "case 13"
    );
}
