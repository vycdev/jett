include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve("a".to_string()), "a".to_string(), "case 1");
    assert_eq!(solve("A".to_string()), "a".to_string(), "case 2");
    assert_eq!(
        solve("camelCase".to_string()),
        "camel_case".to_string(),
        "case 3"
    );
    assert_eq!(
        solve("HTTPServer".to_string()),
        "http_server".to_string(),
        "case 4"
    );
    assert_eq!(solve("XML".to_string()), "xml".to_string(), "case 5");
    assert_eq!(
        solve("parseURLValue".to_string()),
        "parse_url_value".to_string(),
        "case 6"
    );
    assert_eq!(solve("x2Y".to_string()), "x2_y".to_string(), "case 7");
    assert_eq!(solve("ABc".to_string()), "a_bc".to_string(), "case 8");
    assert_eq!(
        solve("oneTwoThree".to_string()),
        "one_two_three".to_string(),
        "case 9"
    );
    assert_eq!(solve("v123".to_string()), "v123".to_string(), "case 10");
    assert_eq!(
        solve("ABCdEF".to_string()),
        "ab_cd_ef".to_string(),
        "case 11"
    );
}
