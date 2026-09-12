include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(equilibrium_index(&[]), None, "case 0");
    assert_eq!(equilibrium_index(&[0]), Some(0), "case 1");
    assert_eq!(equilibrium_index(&[-7]), Some(0), "case 2");
    assert_eq!(equilibrium_index(&[1, 2, 3, 4]), None, "case 3");
    assert_eq!(equilibrium_index(&[4, 3, 2, 1]), None, "case 4");
    assert_eq!(equilibrium_index(&[2, 2, 2]), Some(1), "case 5");
    assert_eq!(equilibrium_index(&[-3, -2, -1]), None, "case 6");
    assert_eq!(equilibrium_index(&[3, 1, 2, 1, 4]), None, "case 7");
    assert_eq!(equilibrium_index(&[5, -1, 5, -1, 5]), Some(2), "case 8");
    assert_eq!(equilibrium_index(&[0, -1, 2, -3, 4, -5]), None, "case 9");
    assert_eq!(equilibrium_index(&[9, 3, 7, 1, 8, 2, 6, 4, 5]), None, "case 10");
    assert_eq!(equilibrium_index(&[100, -100, 100, 0]), Some(0), "case 11");
    assert_eq!(equilibrium_index(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), Some(0), "case 12");
    assert_eq!(equilibrium_index(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]), None, "case 13");
    assert_eq!(equilibrium_index(&[20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]), None, "case 14");
    assert_eq!(equilibrium_index(&[1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000]), None, "case 15");
    assert_eq!(equilibrium_index(&[100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]), None, "case 16");
    assert_eq!(equilibrium_index(&[1, 3, 5, 2, 2]), Some(2), "case 17");
    assert_eq!(equilibrium_index(&[-7, 1, 5, 2, -4, 3, 0]), Some(3), "case 18");
    assert_eq!(equilibrium_index(&[0, 0, 0]), Some(0), "case 19");
    assert_eq!(equilibrium_index(&[2, -2, 7]), Some(2), "case 20");
    assert_eq!(equilibrium_index(&[7, 2, -2]), Some(0), "case 21");
}
