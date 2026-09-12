include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string(), "".to_string()), true, "case 0");
    assert_eq!(solve("".to_string(), "*".to_string()), true, "case 1");
    assert_eq!(solve("".to_string(), "?".to_string()), false, "case 2");
    assert_eq!(solve("abc".to_string(), "a?c".to_string()), true, "case 3");
    assert_eq!(solve("abc".to_string(), "a*c".to_string()), true, "case 4");
    assert_eq!(
        solve("abbbc".to_string(), "a*b*c".to_string()),
        true,
        "case 5"
    );
    assert_eq!(solve("abc".to_string(), "*d".to_string()), false, "case 6");
    assert_eq!(
        solve("abcd".to_string(), "a*d?".to_string()),
        false,
        "case 7"
    );
    assert_eq!(
        solve("abcdef".to_string(), "*?c*f".to_string()),
        true,
        "case 8"
    );
    assert_eq!(
        solve("aaaab".to_string(), "a*b".to_string()),
        true,
        "case 9"
    );
    assert_eq!(
        solve("abc".to_string(), "**a**?**c**".to_string()),
        true,
        "case 10"
    );
    assert_eq!(solve("ab".to_string(), "a".to_string()), false, "case 11");
    assert_eq!(solve("a.b".to_string(), "a.b".to_string()), true, "case 12");
    assert_eq!(
        solve("ABC".to_string(), "abc".to_string()),
        false,
        "case 13"
    );
}
