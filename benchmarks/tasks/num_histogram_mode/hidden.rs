include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(histogram_mode(&[]), Mode {value: 0, count: 0}, "case 0");
    assert_eq!(histogram_mode(&[0]), Mode {value: 0, count: 1}, "case 1");
    assert_eq!(histogram_mode(&[-7]), Mode {value: -7, count: 1}, "case 2");
    assert_eq!(histogram_mode(&[1, 2, 3, 4]), Mode {value: 1, count: 1}, "case 3");
    assert_eq!(histogram_mode(&[4, 3, 2, 1]), Mode {value: 1, count: 1}, "case 4");
    assert_eq!(histogram_mode(&[2, 2, 2]), Mode {value: 2, count: 3}, "case 5");
    assert_eq!(histogram_mode(&[-3, -2, -1]), Mode {value: -3, count: 1}, "case 6");
    assert_eq!(histogram_mode(&[3, 1, 2, 1, 4]), Mode {value: 1, count: 2}, "case 7");
    assert_eq!(histogram_mode(&[5, -1, 5, -1, 5]), Mode {value: 5, count: 3}, "case 8");
    assert_eq!(histogram_mode(&[0, -1, 2, -3, 4, -5]), Mode {value: -5, count: 1}, "case 9");
    assert_eq!(histogram_mode(&[9, 3, 7, 1, 8, 2, 6, 4, 5]), Mode {value: 1, count: 1}, "case 10");
    assert_eq!(histogram_mode(&[100, -100, 100, 0]), Mode {value: 100, count: 2}, "case 11");
    assert_eq!(histogram_mode(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), Mode {value: 0, count: 20}, "case 12");
    assert_eq!(histogram_mode(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]), Mode {value: 0, count: 1}, "case 13");
    assert_eq!(histogram_mode(&[20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]), Mode {value: 0, count: 1}, "case 14");
    assert_eq!(histogram_mode(&[1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000]), Mode {value: -1000, count: 50}, "case 15");
    assert_eq!(histogram_mode(&[100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]), Mode {value: 1, count: 1}, "case 16");
    assert_eq!(histogram_mode(&[3, 3, 1, 1]), Mode {value: 1, count: 2}, "case 17");
    assert_eq!(histogram_mode(&[-2, -2, -3, -3]), Mode {value: -3, count: 2}, "case 18");
    assert_eq!(histogram_mode(&[5, 1, 5, 1, 5]), Mode {value: 5, count: 3}, "case 19");
    assert_eq!(histogram_mode(&[1000, -1000]), Mode {value: -1000, count: 1}, "case 20");
    assert_eq!(histogram_mode(&[7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7]), Mode {value: 7, count: 100}, "case 21");
}
