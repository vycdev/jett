include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Rejected(TextError::Empty),
        "case 0"
    );
    assert_eq!(
        solve("0".to_string()),
        TextOutcome::Accepted("0".to_string()),
        "case 1"
    );
    assert_eq!(
        solve("-000".to_string()),
        TextOutcome::Accepted("0".to_string()),
        "case 2"
    );
    assert_eq!(
        solve("+0012".to_string()),
        TextOutcome::Accepted("12".to_string()),
        "case 3"
    );
    assert_eq!(
        solve("-012".to_string()),
        TextOutcome::Accepted("-12".to_string()),
        "case 4"
    );
    assert_eq!(
        solve("123".to_string()),
        TextOutcome::Accepted("123".to_string()),
        "case 5"
    );
    assert_eq!(
        solve("+".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 6"
    );
    assert_eq!(
        solve("--1".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve(" 1".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 8"
    );
    assert_eq!(
        solve("1.0".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 9"
    );
    assert_eq!(
        solve("0x0".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 10"
    );
    assert_eq!(solve("9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999".to_string()), TextOutcome::Accepted("9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999".to_string()), "case 11");
    assert_eq!(solve("-0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001".to_string()), TextOutcome::Accepted("-1".to_string()), "case 12");
}
