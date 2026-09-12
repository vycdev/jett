include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve(" ".to_string()), "".to_string(), "case 1");
    assert_eq!(solve("a".to_string()), "a".to_string(), "case 2");
    assert_eq!(solve("a b".to_string()), "b a".to_string(), "case 3");
    assert_eq!(solve(" a  b c ".to_string()), "c b a".to_string(), "case 4");
    assert_eq!(
        solve("one two three".to_string()),
        "three two one".to_string(),
        "case 5"
    );
    assert_eq!(solve("a a b".to_string()), "b a a".to_string(), "case 6");
    assert_eq!(solve("! ?".to_string()), "? !".to_string(), "case 7");
    assert_eq!(solve("a\tb c".to_string()), "c a\tb".to_string(), "case 8");
    assert_eq!(solve("word  ".to_string()), "word".to_string(), "case 9");
    assert_eq!(solve("  x".to_string()), "x".to_string(), "case 10");
    assert_eq!(
        solve("ab cd ef gh".to_string()),
        "gh ef cd ab".to_string(),
        "case 11"
    );
}
