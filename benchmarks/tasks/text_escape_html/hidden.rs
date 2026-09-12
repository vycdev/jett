include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve("abc".to_string()), "abc".to_string(), "case 1");
    assert_eq!(solve("&".to_string()), "&amp;".to_string(), "case 2");
    assert_eq!(solve("<>".to_string()), "&lt;&gt;".to_string(), "case 3");
    assert_eq!(
        solve("\"'".to_string()),
        "&quot;&#39;".to_string(),
        "case 4"
    );
    assert_eq!(
        solve("&amp;".to_string()),
        "&amp;amp;".to_string(),
        "case 5"
    );
    assert_eq!(
        solve("a&b<c".to_string()),
        "a&amp;b&lt;c".to_string(),
        "case 6"
    );
    assert_eq!(solve("&&".to_string()), "&amp;&amp;".to_string(), "case 7");
    assert_eq!(solve("\n\t".to_string()), "\n\t".to_string(), "case 8");
    assert_eq!(solve("/a=1".to_string()), "/a=1".to_string(), "case 9");
    assert_eq!(
        solve("<a x=\"b\">".to_string()),
        "&lt;a x=&quot;b&quot;&gt;".to_string(),
        "case 10"
    );
    assert_eq!(
        solve("'&'".to_string()),
        "&#39;&amp;&#39;".to_string(),
        "case 11"
    );
}
