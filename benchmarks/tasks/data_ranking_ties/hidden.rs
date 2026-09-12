include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            selected: 0,
            rank_sum: 0,
            tie_groups: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            selected: 0,
            rank_sum: 0,
            tie_groups: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            selected: 0,
            rank_sum: 0,
            tie_groups: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                score: 8,
                penalty: 3
            }],
            2
        ),
        Report {
            selected: 1,
            rank_sum: 1,
            tie_groups: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 6,
                    penalty: 3
                },
                Entry {
                    score: 7,
                    penalty: 5
                }
            ],
            3
        ),
        Report {
            selected: 2,
            rank_sum: 3,
            tie_groups: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 11,
                    penalty: 1
                },
                Entry {
                    score: 0,
                    penalty: 6
                },
                Entry {
                    score: (-2),
                    penalty: 5
                }
            ],
            4
        ),
        Report {
            selected: 3,
            rank_sum: 6,
            tie_groups: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 10,
                    penalty: 2
                },
                Entry {
                    score: 2,
                    penalty: 3
                },
                Entry {
                    score: 0,
                    penalty: 4
                },
                Entry {
                    score: 11,
                    penalty: 2
                }
            ],
            5
        ),
        Report {
            selected: 4,
            rank_sum: 10,
            tie_groups: 0
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 10,
                    penalty: 3
                },
                Entry {
                    score: 5,
                    penalty: 5
                },
                Entry {
                    score: 2,
                    penalty: 6
                },
                Entry {
                    score: 3,
                    penalty: 1
                },
                Entry {
                    score: 1,
                    penalty: 1
                }
            ],
            6
        ),
        Report {
            selected: 5,
            rank_sum: 15,
            tie_groups: 0
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 5,
                    penalty: 6
                },
                Entry {
                    score: (-2),
                    penalty: 5
                },
                Entry {
                    score: 11,
                    penalty: 6
                },
                Entry {
                    score: 2,
                    penalty: 4
                },
                Entry {
                    score: 10,
                    penalty: 6
                },
                Entry {
                    score: 6,
                    penalty: 5
                }
            ],
            7
        ),
        Report {
            selected: 6,
            rank_sum: 21,
            tie_groups: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 5,
                    penalty: 1
                },
                Entry {
                    score: 1,
                    penalty: 1
                },
                Entry {
                    score: (-1),
                    penalty: 1
                },
                Entry {
                    score: 8,
                    penalty: 4
                },
                Entry {
                    score: 7,
                    penalty: 4
                },
                Entry {
                    score: 5,
                    penalty: 4
                },
                Entry {
                    score: 11,
                    penalty: 3
                }
            ],
            8
        ),
        Report {
            selected: 7,
            rank_sum: 28,
            tie_groups: 0
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: (-3),
                    penalty: 3
                },
                Entry {
                    score: 7,
                    penalty: 4
                },
                Entry {
                    score: 7,
                    penalty: 1
                },
                Entry {
                    score: (-2),
                    penalty: 3
                },
                Entry {
                    score: (-1),
                    penalty: 4
                },
                Entry {
                    score: 4,
                    penalty: 5
                },
                Entry {
                    score: 8,
                    penalty: 4
                },
                Entry {
                    score: (-2),
                    penalty: 6
                }
            ],
            1
        ),
        Report {
            selected: 1,
            rank_sum: 1,
            tie_groups: 0
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 10,
                    penalty: 2
                },
                Entry {
                    score: 11,
                    penalty: 2
                },
                Entry {
                    score: 2,
                    penalty: 2
                },
                Entry {
                    score: 1,
                    penalty: 2
                },
                Entry {
                    score: 1,
                    penalty: 5
                },
                Entry {
                    score: 4,
                    penalty: 2
                },
                Entry {
                    score: 2,
                    penalty: 1
                },
                Entry {
                    score: (-3),
                    penalty: 4
                },
                Entry {
                    score: 2,
                    penalty: 2
                }
            ],
            2
        ),
        Report {
            selected: 2,
            rank_sum: 3,
            tie_groups: 1
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 7,
                    penalty: 1
                },
                Entry {
                    score: (-3),
                    penalty: 1
                },
                Entry {
                    score: 5,
                    penalty: 6
                },
                Entry {
                    score: 10,
                    penalty: 1
                },
                Entry {
                    score: 8,
                    penalty: 3
                },
                Entry {
                    score: 6,
                    penalty: 3
                },
                Entry {
                    score: 2,
                    penalty: 3
                },
                Entry {
                    score: 5,
                    penalty: 3
                },
                Entry {
                    score: (-1),
                    penalty: 2
                },
                Entry {
                    score: 1,
                    penalty: 2
                }
            ],
            3
        ),
        Report {
            selected: 3,
            rank_sum: 6,
            tie_groups: 0
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 5,
                    penalty: 3
                },
                Entry {
                    score: 3,
                    penalty: 6
                },
                Entry {
                    score: 11,
                    penalty: 1
                },
                Entry {
                    score: 8,
                    penalty: 6
                },
                Entry {
                    score: 4,
                    penalty: 2
                },
                Entry {
                    score: 1,
                    penalty: 1
                },
                Entry {
                    score: 3,
                    penalty: 2
                },
                Entry {
                    score: 5,
                    penalty: 6
                },
                Entry {
                    score: 6,
                    penalty: 5
                },
                Entry {
                    score: 9,
                    penalty: 5
                },
                Entry {
                    score: 7,
                    penalty: 2
                }
            ],
            4
        ),
        Report {
            selected: 4,
            rank_sum: 10,
            tie_groups: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 3,
                    penalty: 2
                },
                Entry {
                    score: 7,
                    penalty: 2
                },
                Entry {
                    score: 4,
                    penalty: 6
                },
                Entry {
                    score: 6,
                    penalty: 3
                },
                Entry {
                    score: 11,
                    penalty: 3
                },
                Entry {
                    score: 0,
                    penalty: 5
                },
                Entry {
                    score: 2,
                    penalty: 2
                },
                Entry {
                    score: 10,
                    penalty: 3
                },
                Entry {
                    score: (-1),
                    penalty: 5
                },
                Entry {
                    score: (-1),
                    penalty: 1
                },
                Entry {
                    score: 11,
                    penalty: 6
                },
                Entry {
                    score: (-3),
                    penalty: 4
                }
            ],
            5
        ),
        Report {
            selected: 5,
            rank_sum: 15,
            tie_groups: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            selected: 0,
            rank_sum: 0,
            tie_groups: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                score: (-1),
                penalty: 5
            }],
            7
        ),
        Report {
            selected: 1,
            rank_sum: 1,
            tie_groups: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 4,
                    penalty: 1
                },
                Entry {
                    score: 8,
                    penalty: 4
                }
            ],
            8
        ),
        Report {
            selected: 2,
            rank_sum: 3,
            tie_groups: 0
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 7,
                    penalty: 3
                },
                Entry {
                    score: (-1),
                    penalty: 4
                },
                Entry {
                    score: 3,
                    penalty: 4
                }
            ],
            1
        ),
        Report {
            selected: 1,
            rank_sum: 1,
            tie_groups: 0
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 11,
                    penalty: 3
                },
                Entry {
                    score: 5,
                    penalty: 2
                },
                Entry {
                    score: 4,
                    penalty: 3
                },
                Entry {
                    score: 4,
                    penalty: 2
                }
            ],
            2
        ),
        Report {
            selected: 2,
            rank_sum: 3,
            tie_groups: 0
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 1,
                    penalty: 5
                },
                Entry {
                    score: 5,
                    penalty: 4
                },
                Entry {
                    score: (-3),
                    penalty: 3
                },
                Entry {
                    score: 5,
                    penalty: 5
                },
                Entry {
                    score: (-3),
                    penalty: 6
                }
            ],
            3
        ),
        Report {
            selected: 3,
            rank_sum: 6,
            tie_groups: 0
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 4,
                    penalty: 4
                },
                Entry {
                    score: 1,
                    penalty: 6
                },
                Entry {
                    score: 10,
                    penalty: 1
                },
                Entry {
                    score: (-3),
                    penalty: 1
                },
                Entry {
                    score: 4,
                    penalty: 1
                },
                Entry {
                    score: 4,
                    penalty: 6
                }
            ],
            4
        ),
        Report {
            selected: 4,
            rank_sum: 10,
            tie_groups: 0
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 1,
                    penalty: 6
                },
                Entry {
                    score: 6,
                    penalty: 5
                },
                Entry {
                    score: 6,
                    penalty: 5
                },
                Entry {
                    score: 0,
                    penalty: 5
                },
                Entry {
                    score: 0,
                    penalty: 2
                },
                Entry {
                    score: 11,
                    penalty: 1
                },
                Entry {
                    score: (-1),
                    penalty: 4
                }
            ],
            5
        ),
        Report {
            selected: 5,
            rank_sum: 14,
            tie_groups: 1
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 6,
                    penalty: 1
                },
                Entry {
                    score: (-1),
                    penalty: 1
                },
                Entry {
                    score: (-3),
                    penalty: 4
                },
                Entry {
                    score: 9,
                    penalty: 5
                },
                Entry {
                    score: (-1),
                    penalty: 2
                },
                Entry {
                    score: 2,
                    penalty: 2
                },
                Entry {
                    score: (-2),
                    penalty: 3
                },
                Entry {
                    score: (-1),
                    penalty: 6
                }
            ],
            6
        ),
        Report {
            selected: 6,
            rank_sum: 21,
            tie_groups: 0
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 9,
                    penalty: 1
                },
                Entry {
                    score: 9,
                    penalty: 1
                },
                Entry {
                    score: 8,
                    penalty: 1
                }
            ],
            2
        ),
        Report {
            selected: 2,
            rank_sum: 2,
            tie_groups: 1
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 0,
                    penalty: 2
                },
                Entry {
                    score: 0,
                    penalty: 1
                }
            ],
            1
        ),
        Report {
            selected: 1,
            rank_sum: 1,
            tie_groups: 0
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    score: 9,
                    penalty: 1
                },
                Entry {
                    score: 9,
                    penalty: 1
                },
                Entry {
                    score: 9,
                    penalty: 2
                },
                Entry {
                    score: 8,
                    penalty: 0
                }
            ],
            3
        ),
        Report {
            selected: 3,
            rank_sum: 5,
            tie_groups: 1
        },
        "fixture 26"
    );
}
