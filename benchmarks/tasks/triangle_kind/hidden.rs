include!("solution.rs");

#[test]
fn hidden_cases() {
    assert_eq!(triangle_kind(3, 3, 3), "equilateral");
    assert_eq!(triangle_kind(5, 5, 8), "isosceles");
    assert_eq!(triangle_kind(3, 4, 5), "scalene");
    assert_eq!(triangle_kind(1, 2, 3), "invalid");
    assert_eq!(triangle_kind(0, 4, 4), "invalid");
    assert_eq!(triangle_kind(-1, 2, 2), "invalid");
    assert_eq!(triangle_kind(10, 3, 3), "invalid");
}
