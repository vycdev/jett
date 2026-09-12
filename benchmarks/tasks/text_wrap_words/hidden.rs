include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string(), 5), "".to_string(), "case 0");
    assert_eq!(solve("   ".to_string(), 3), "".to_string(), "case 1");
    assert_eq!(
        solve("a b c".to_string(), 3),
        "a b\nc".to_string(),
        "case 2"
    );
    assert_eq!(
        solve("a b c".to_string(), 1),
        "a\nb\nc".to_string(),
        "case 3"
    );
    assert_eq!(
        solve("longword x".to_string(), 3),
        "longword\nx".to_string(),
        "case 4"
    );
    assert_eq!(
        solve("one two three".to_string(), 7),
        "one two\nthree".to_string(),
        "case 5"
    );
    assert_eq!(
        solve(" one  two ".to_string(), 80),
        "one two".to_string(),
        "case 6"
    );
    assert_eq!(
        solve("ab cd ef".to_string(), 5),
        "ab cd\nef".to_string(),
        "case 7"
    );
    assert_eq!(
        solve("abc d".to_string(), 3),
        "abc\nd".to_string(),
        "case 8"
    );
    assert_eq!(solve("a bc".to_string(), 4), "a bc".to_string(), "case 9");
    assert_eq!(solve("a bc".to_string(), 3), "a\nbc".to_string(), "case 10");
    assert_eq!(
        solve("x longword y".to_string(), 3),
        "x\nlongword\ny".to_string(),
        "case 11"
    );
}
