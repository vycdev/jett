include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(staircase_blocked(0, &[]), 1, "case 0");
    assert_eq!(staircase_blocked(1, &[]), 1, "case 1");
    assert_eq!(staircase_blocked(1, &[1]), 0, "case 2");
    assert_eq!(staircase_blocked(2, &[1]), 1, "case 3");
    assert_eq!(staircase_blocked(2, &[2]), 0, "case 4");
    assert_eq!(staircase_blocked(3, &[]), 3, "case 5");
    assert_eq!(staircase_blocked(4, &[2]), 1, "case 6");
    assert_eq!(staircase_blocked(5, &[2, 3]), 0, "case 7");
    assert_eq!(staircase_blocked(6, &[5, 1]), 2, "case 8");
    assert_eq!(staircase_blocked(10, &[4, 4, 7]), 6, "case 9");
    assert_eq!(staircase_blocked(20, &[]), 10946, "case 10");
    assert_eq!(staircase_blocked(40, &[]), 165580141, "case 11");
    assert_eq!(staircase_blocked(40, &[39]), 63245986, "case 12");
    assert_eq!(staircase_blocked(8, &[1, 2]), 0, "case 13");
}
