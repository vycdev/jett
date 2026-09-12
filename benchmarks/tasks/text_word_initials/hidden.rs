include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve("  ".to_string()), "".to_string(), "case 1");
    assert_eq!(solve("alice".to_string()), "A".to_string(), "case 2");
    assert_eq!(solve("alice bob".to_string()), "AB".to_string(), "case 3");
    assert_eq!(
        solve("  alice   bob ".to_string()),
        "AB".to_string(),
        "case 4"
    );
    assert_eq!(solve("a b c".to_string()), "ABC".to_string(), "case 5");
    assert_eq!(solve("1 one".to_string()), "1O".to_string(), "case 6");
    assert_eq!(solve("! bang".to_string()), "!B".to_string(), "case 7");
    assert_eq!(solve("mixed CASE".to_string()), "MC".to_string(), "case 8");
    assert_eq!(solve("a\tb c".to_string()), "AC".to_string(), "case 9");
    assert_eq!(solve("z Z".to_string()), "ZZ".to_string(), "case 10");
    assert_eq!(
        solve("foo-bar baz".to_string()),
        "FB".to_string(),
        "case 11"
    );
}
