include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("00:00:00".to_string()),
        TextOutcome::Accepted(0),
        "case 0"
    );
    assert_eq!(
        solve("23:59:59".to_string()),
        TextOutcome::Accepted(86399),
        "case 1"
    );
    assert_eq!(
        solve("12:34:56".to_string()),
        TextOutcome::Accepted(45296),
        "case 2"
    );
    assert_eq!(
        solve("01:02:03".to_string()),
        TextOutcome::Accepted(3723),
        "case 3"
    );
    assert_eq!(
        solve("24:00:00".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 4"
    );
    assert_eq!(
        solve("00:60:00".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 5"
    );
    assert_eq!(
        solve("00:00:60".to_string()),
        TextOutcome::Rejected(TextError::Range),
        "case 6"
    );
    assert_eq!(
        solve("0:00:00".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 7"
    );
    assert_eq!(
        solve("00-00-00".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 8"
    );
    assert_eq!(
        solve("aa:00:00".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 9"
    );
    assert_eq!(
        solve("99:xx:00".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 10"
    );
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Rejected(TextError::Malformed),
        "case 11"
    );
}
