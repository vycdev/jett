include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextOutcome::Rejected(TextError::Empty),
        "case 0"
    );
    assert_eq!(
        solve("  ".to_string()),
        TextOutcome::Rejected(TextError::Empty),
        "case 1"
    );
    assert_eq!(
        solve("a".to_string()),
        TextOutcome::Accepted("a".to_string()),
        "case 2"
    );
    assert_eq!(
        solve("a bb c".to_string()),
        TextOutcome::Accepted("bb".to_string()),
        "case 3"
    );
    assert_eq!(
        solve("ab cd".to_string()),
        TextOutcome::Accepted("ab".to_string()),
        "case 4"
    );
    assert_eq!(
        solve(" xxx yy ".to_string()),
        TextOutcome::Accepted("xxx".to_string()),
        "case 5"
    );
    assert_eq!(
        solve("a\tb zz".to_string()),
        TextOutcome::Accepted("a\tb".to_string()),
        "case 6"
    );
    assert_eq!(
        solve("! ??".to_string()),
        TextOutcome::Accepted("??".to_string()),
        "case 7"
    );
    assert_eq!(
        solve("long short".to_string()),
        TextOutcome::Accepted("short".to_string()),
        "case 8"
    );
    assert_eq!(
        solve("same same".to_string()),
        TextOutcome::Accepted("same".to_string()),
        "case 9"
    );
    assert_eq!(
        solve("   end".to_string()),
        TextOutcome::Accepted("end".to_string()),
        "case 10"
    );
    assert_eq!(
        solve("ab abc abcd".to_string()),
        TextOutcome::Accepted("abcd".to_string()),
        "case 11"
    );
}
