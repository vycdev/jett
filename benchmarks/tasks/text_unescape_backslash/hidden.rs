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
        solve("\\n".to_string()),
        TextOutcome::Accepted("\n".to_string()),
        "case 2"
    );
    assert_eq!(
        solve("\\t\\r".to_string()),
        TextOutcome::Accepted("\t\r".to_string()),
        "case 3"
    );
    assert_eq!(
        solve("\\\\".to_string()),
        TextOutcome::Accepted("\\".to_string()),
        "case 4"
    );
    assert_eq!(
        solve("\\\"".to_string()),
        TextOutcome::Accepted("\"".to_string()),
        "case 5"
    );
    assert_eq!(
        solve("a\\nb".to_string()),
        TextOutcome::Accepted("a\nb".to_string()),
        "case 6"
    );
    assert_eq!(
        solve("\\".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve("x\\q".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 8"
    );
    assert_eq!(
        solve("\\0".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 9"
    );
    assert_eq!(
        solve("\\\\n".to_string()),
        TextOutcome::Accepted("\\n".to_string()),
        "case 10"
    );
    assert_eq!(
        solve("a\nb".to_string()),
        TextOutcome::Accepted("a\nb".to_string()),
        "case 11"
    );
}
