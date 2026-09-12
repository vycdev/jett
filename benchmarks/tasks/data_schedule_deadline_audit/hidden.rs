include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 5
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 1
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                duration: 8,
                deadline: 23,
                penalty: 3
            }],
            2
        ),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 10
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 4,
                    deadline: 18,
                    penalty: 3
                },
                Entry {
                    duration: 7,
                    deadline: 20,
                    penalty: 3
                }
            ],
            3
        ),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 14
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 1,
                    deadline: 0,
                    penalty: 2
                },
                Entry {
                    duration: 2,
                    deadline: 2,
                    penalty: 4
                },
                Entry {
                    duration: 4,
                    deadline: 11,
                    penalty: 3
                }
            ],
            4
        ),
        Report {
            late: 2,
            weighted_tardiness: 30,
            finish: 11
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 6,
                    deadline: 18,
                    penalty: 2
                },
                Entry {
                    duration: 8,
                    deadline: 2,
                    penalty: 2
                },
                Entry {
                    duration: 7,
                    deadline: 27,
                    penalty: 3
                },
                Entry {
                    duration: 1,
                    deadline: 17,
                    penalty: 3
                }
            ],
            5
        ),
        Report {
            late: 2,
            weighted_tardiness: 64,
            finish: 27
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 3,
                    deadline: 12,
                    penalty: 1
                },
                Entry {
                    duration: 4,
                    deadline: 8,
                    penalty: 1
                },
                Entry {
                    duration: 7,
                    deadline: 16,
                    penalty: 2
                },
                Entry {
                    duration: 2,
                    deadline: 24,
                    penalty: 4
                },
                Entry {
                    duration: 6,
                    deadline: 14,
                    penalty: 1
                }
            ],
            6
        ),
        Report {
            late: 3,
            weighted_tardiness: 27,
            finish: 28
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 7,
                    deadline: 19,
                    penalty: 2
                },
                Entry {
                    duration: 1,
                    deadline: 7,
                    penalty: 3
                },
                Entry {
                    duration: 2,
                    deadline: 14,
                    penalty: 2
                },
                Entry {
                    duration: 1,
                    deadline: 0,
                    penalty: 4
                },
                Entry {
                    duration: 7,
                    deadline: 21,
                    penalty: 4
                },
                Entry {
                    duration: 1,
                    deadline: 17,
                    penalty: 4
                }
            ],
            7
        ),
        Report {
            late: 5,
            weighted_tardiness: 154,
            finish: 26
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 3,
                    deadline: 29,
                    penalty: 3
                },
                Entry {
                    duration: 1,
                    deadline: 1,
                    penalty: 3
                },
                Entry {
                    duration: 4,
                    deadline: 21,
                    penalty: 4
                },
                Entry {
                    duration: 3,
                    deadline: 21,
                    penalty: 1
                },
                Entry {
                    duration: 8,
                    deadline: 3,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 26,
                    penalty: 4
                },
                Entry {
                    duration: 2,
                    deadline: 15,
                    penalty: 3
                }
            ],
            8
        ),
        Report {
            late: 4,
            weighted_tardiness: 172,
            finish: 32
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 7,
                    deadline: 24,
                    penalty: 2
                },
                Entry {
                    duration: 2,
                    deadline: 24,
                    penalty: 3
                },
                Entry {
                    duration: 4,
                    deadline: 21,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 27,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 13,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 8,
                    penalty: 3
                },
                Entry {
                    duration: 6,
                    deadline: 15,
                    penalty: 2
                },
                Entry {
                    duration: 1,
                    deadline: 10,
                    penalty: 1
                }
            ],
            1
        ),
        Report {
            late: 4,
            weighted_tardiness: 114,
            finish: 30
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 8,
                    deadline: 0,
                    penalty: 4
                },
                Entry {
                    duration: 6,
                    deadline: 6,
                    penalty: 1
                },
                Entry {
                    duration: 2,
                    deadline: 15,
                    penalty: 1
                },
                Entry {
                    duration: 1,
                    deadline: 4,
                    penalty: 1
                },
                Entry {
                    duration: 2,
                    deadline: 20,
                    penalty: 2
                },
                Entry {
                    duration: 6,
                    deadline: 12,
                    penalty: 3
                },
                Entry {
                    duration: 2,
                    deadline: 10,
                    penalty: 3
                },
                Entry {
                    duration: 5,
                    deadline: 29,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 6,
                    penalty: 1
                }
            ],
            2
        ),
        Report {
            late: 9,
            weighted_tardiness: 218,
            finish: 37
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 5,
                    deadline: 6,
                    penalty: 3
                },
                Entry {
                    duration: 5,
                    deadline: 16,
                    penalty: 4
                },
                Entry {
                    duration: 7,
                    deadline: 29,
                    penalty: 1
                },
                Entry {
                    duration: 4,
                    deadline: 22,
                    penalty: 4
                },
                Entry {
                    duration: 8,
                    deadline: 6,
                    penalty: 1
                },
                Entry {
                    duration: 5,
                    deadline: 29,
                    penalty: 1
                },
                Entry {
                    duration: 7,
                    deadline: 7,
                    penalty: 2
                },
                Entry {
                    duration: 3,
                    deadline: 24,
                    penalty: 2
                },
                Entry {
                    duration: 4,
                    deadline: 13,
                    penalty: 2
                },
                Entry {
                    duration: 3,
                    deadline: 21,
                    penalty: 2
                }
            ],
            3
        ),
        Report {
            late: 8,
            weighted_tardiness: 310,
            finish: 54
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 8,
                    deadline: 14,
                    penalty: 3
                },
                Entry {
                    duration: 6,
                    deadline: 8,
                    penalty: 3
                },
                Entry {
                    duration: 4,
                    deadline: 6,
                    penalty: 1
                },
                Entry {
                    duration: 6,
                    deadline: 4,
                    penalty: 2
                },
                Entry {
                    duration: 5,
                    deadline: 5,
                    penalty: 2
                },
                Entry {
                    duration: 2,
                    deadline: 4,
                    penalty: 1
                },
                Entry {
                    duration: 8,
                    deadline: 28,
                    penalty: 4
                },
                Entry {
                    duration: 1,
                    deadline: 12,
                    penalty: 4
                },
                Entry {
                    duration: 3,
                    deadline: 16,
                    penalty: 1
                },
                Entry {
                    duration: 8,
                    deadline: 2,
                    penalty: 4
                },
                Entry {
                    duration: 7,
                    deadline: 21,
                    penalty: 3
                }
            ],
            4
        ),
        Report {
            late: 10,
            weighted_tardiness: 735,
            finish: 62
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 2,
                    deadline: 5,
                    penalty: 4
                },
                Entry {
                    duration: 1,
                    deadline: 13,
                    penalty: 4
                },
                Entry {
                    duration: 3,
                    deadline: 29,
                    penalty: 3
                },
                Entry {
                    duration: 1,
                    deadline: 17,
                    penalty: 2
                },
                Entry {
                    duration: 8,
                    deadline: 14,
                    penalty: 3
                },
                Entry {
                    duration: 1,
                    deadline: 15,
                    penalty: 2
                },
                Entry {
                    duration: 4,
                    deadline: 9,
                    penalty: 3
                },
                Entry {
                    duration: 7,
                    deadline: 16,
                    penalty: 1
                },
                Entry {
                    duration: 5,
                    deadline: 9,
                    penalty: 2
                },
                Entry {
                    duration: 1,
                    deadline: 21,
                    penalty: 1
                },
                Entry {
                    duration: 8,
                    deadline: 13,
                    penalty: 3
                },
                Entry {
                    duration: 5,
                    deadline: 25,
                    penalty: 4
                }
            ],
            5
        ),
        Report {
            late: 9,
            weighted_tardiness: 378,
            finish: 51
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 6
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                duration: 1,
                deadline: 16,
                penalty: 1
            }],
            7
        ),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 8
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 2,
                    deadline: 11,
                    penalty: 4
                },
                Entry {
                    duration: 1,
                    deadline: 28,
                    penalty: 2
                }
            ],
            8
        ),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 11
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 8,
                    deadline: 23,
                    penalty: 3
                },
                Entry {
                    duration: 5,
                    deadline: 21,
                    penalty: 1
                },
                Entry {
                    duration: 3,
                    deadline: 19,
                    penalty: 1
                }
            ],
            1
        ),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 17
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 4,
                    deadline: 18,
                    penalty: 2
                },
                Entry {
                    duration: 4,
                    deadline: 7,
                    penalty: 3
                },
                Entry {
                    duration: 1,
                    deadline: 22,
                    penalty: 2
                },
                Entry {
                    duration: 8,
                    deadline: 16,
                    penalty: 1
                }
            ],
            2
        ),
        Report {
            late: 2,
            weighted_tardiness: 12,
            finish: 19
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 3,
                    deadline: 2,
                    penalty: 1
                },
                Entry {
                    duration: 1,
                    deadline: 29,
                    penalty: 4
                },
                Entry {
                    duration: 5,
                    deadline: 24,
                    penalty: 1
                },
                Entry {
                    duration: 3,
                    deadline: 28,
                    penalty: 2
                },
                Entry {
                    duration: 2,
                    deadline: 10,
                    penalty: 2
                }
            ],
            3
        ),
        Report {
            late: 2,
            weighted_tardiness: 18,
            finish: 17
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 3,
                    deadline: 3,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 4,
                    penalty: 2
                },
                Entry {
                    duration: 6,
                    deadline: 9,
                    penalty: 4
                },
                Entry {
                    duration: 1,
                    deadline: 7,
                    penalty: 4
                },
                Entry {
                    duration: 7,
                    deadline: 2,
                    penalty: 3
                },
                Entry {
                    duration: 3,
                    deadline: 0,
                    penalty: 4
                }
            ],
            4
        ),
        Report {
            late: 6,
            weighted_tardiness: 266,
            finish: 27
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 1,
                    deadline: 26,
                    penalty: 4
                },
                Entry {
                    duration: 1,
                    deadline: 5,
                    penalty: 2
                },
                Entry {
                    duration: 1,
                    deadline: 15,
                    penalty: 4
                },
                Entry {
                    duration: 5,
                    deadline: 3,
                    penalty: 4
                },
                Entry {
                    duration: 8,
                    deadline: 15,
                    penalty: 1
                },
                Entry {
                    duration: 7,
                    deadline: 0,
                    penalty: 2
                },
                Entry {
                    duration: 2,
                    deadline: 7,
                    penalty: 1
                }
            ],
            5
        ),
        Report {
            late: 5,
            weighted_tardiness: 129,
            finish: 30
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 6,
                    deadline: 10,
                    penalty: 4
                },
                Entry {
                    duration: 2,
                    deadline: 10,
                    penalty: 4
                },
                Entry {
                    duration: 2,
                    deadline: 8,
                    penalty: 1
                },
                Entry {
                    duration: 2,
                    deadline: 28,
                    penalty: 3
                },
                Entry {
                    duration: 5,
                    deadline: 15,
                    penalty: 2
                },
                Entry {
                    duration: 7,
                    deadline: 11,
                    penalty: 3
                },
                Entry {
                    duration: 8,
                    deadline: 2,
                    penalty: 2
                },
                Entry {
                    duration: 7,
                    deadline: 2,
                    penalty: 2
                }
            ],
            6
        ),
        Report {
            late: 7,
            weighted_tardiness: 263,
            finish: 45
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[Entry {
                duration: 2,
                deadline: 5,
                penalty: 1
            }],
            3
        ),
        Report {
            late: 0,
            weighted_tardiness: 0,
            finish: 5
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 1,
                    deadline: 0,
                    penalty: 2
                },
                Entry {
                    duration: 3,
                    deadline: 4,
                    penalty: 5
                }
            ],
            0
        ),
        Report {
            late: 1,
            weighted_tardiness: 2,
            finish: 4
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    duration: 3,
                    deadline: 5,
                    penalty: 2
                },
                Entry {
                    duration: 4,
                    deadline: 6,
                    penalty: 3
                },
                Entry {
                    duration: 1,
                    deadline: 20,
                    penalty: 9
                }
            ],
            2
        ),
        Report {
            late: 1,
            weighted_tardiness: 9,
            finish: 10
        },
        "fixture 26"
    );
}
