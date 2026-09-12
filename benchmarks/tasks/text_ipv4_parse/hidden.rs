include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("0.0.0.0".to_string()),
        TextOutcome::Accepted(0),
        "case 0"
    );
    assert_eq!(
        solve("255.255.255.255".to_string()),
        TextOutcome::Accepted(4294967295),
        "case 1"
    );
    assert_eq!(
        solve("127.0.0.1".to_string()),
        TextOutcome::Accepted(2130706433),
        "case 2"
    );
    assert_eq!(
        solve("192.168.1.1".to_string()),
        TextOutcome::Accepted(3232235777),
        "case 3"
    );
    assert_eq!(
        solve("1.2.3.4".to_string()),
        TextOutcome::Accepted(16909060),
        "case 4"
    );
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 5"
    );
    assert_eq!(
        solve("1.2.3".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 6"
    );
    assert_eq!(
        solve("1.2.3.4.5".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve("01.2.3.4".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 8"
    );
    assert_eq!(
        solve("256.0.0.1".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 9"
    );
    assert_eq!(
        solve("1..2.3".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 10"
    );
    assert_eq!(
        solve("1.2.3.-1".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 11"
    );
    assert_eq!(
        solve("1.2.3.4 ".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 12"
    );
}
