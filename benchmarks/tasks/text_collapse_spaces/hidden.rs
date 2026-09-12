include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve(" ".to_string()), "".to_string(), "case 1");
    assert_eq!(solve("   ".to_string()), "".to_string(), "case 2");
    assert_eq!(solve("a".to_string()), "a".to_string(), "case 3");
    assert_eq!(solve(" a ".to_string()), "a".to_string(), "case 4");
    assert_eq!(solve("a  b".to_string()), "a b".to_string(), "case 5");
    assert_eq!(solve(" a  b c ".to_string()), "a b c".to_string(), "case 6");
    assert_eq!(solve("a\tb".to_string()), "a\tb".to_string(), "case 7");
    assert_eq!(solve(" \t ".to_string()), "\t".to_string(), "case 8");
    assert_eq!(solve("a\n  b".to_string()), "a\n b".to_string(), "case 9");
    assert_eq!(solve("!  ?".to_string()), "! ?".to_string(), "case 10");
    assert_eq!(
        solve(" x   x  x ".to_string()),
        "x x x".to_string(),
        "case 11"
    );
}
