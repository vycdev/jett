include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            total: 0,
            lowest_balance: 0,
            first_overdraw: (-1)
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            total: 0,
            lowest_balance: 5,
            first_overdraw: (-1)
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            total: 0,
            lowest_balance: 1,
            first_overdraw: (-1)
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                account: 3,
                delta: 8
            }],
            2
        ),
        Report {
            total: 10,
            lowest_balance: 10,
            first_overdraw: (-1)
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 1,
                    delta: 6
                },
                Entry {
                    account: 3,
                    delta: 7
                }
            ],
            3
        ),
        Report {
            total: 19,
            lowest_balance: 9,
            first_overdraw: (-1)
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 2,
                    delta: 11
                },
                Entry {
                    account: 0,
                    delta: 0
                },
                Entry {
                    account: 0,
                    delta: (-2)
                }
            ],
            4
        ),
        Report {
            total: 17,
            lowest_balance: 2,
            first_overdraw: (-1)
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 3,
                    delta: 10
                },
                Entry {
                    account: 2,
                    delta: 2
                },
                Entry {
                    account: 4,
                    delta: 0
                },
                Entry {
                    account: 0,
                    delta: 11
                }
            ],
            5
        ),
        Report {
            total: 43,
            lowest_balance: 5,
            first_overdraw: (-1)
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 3,
                    delta: 10
                },
                Entry {
                    account: 0,
                    delta: 5
                },
                Entry {
                    account: 4,
                    delta: 2
                },
                Entry {
                    account: 1,
                    delta: 3
                },
                Entry {
                    account: 1,
                    delta: 1
                }
            ],
            6
        ),
        Report {
            total: 45,
            lowest_balance: 8,
            first_overdraw: (-1)
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 3,
                    delta: 5
                },
                Entry {
                    account: 1,
                    delta: (-2)
                },
                Entry {
                    account: 3,
                    delta: 11
                },
                Entry {
                    account: 4,
                    delta: 2
                },
                Entry {
                    account: 0,
                    delta: 10
                },
                Entry {
                    account: 3,
                    delta: 6
                }
            ],
            7
        ),
        Report {
            total: 60,
            lowest_balance: 5,
            first_overdraw: (-1)
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 1,
                    delta: 5
                },
                Entry {
                    account: 1,
                    delta: 1
                },
                Entry {
                    account: 3,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: 8
                },
                Entry {
                    account: 3,
                    delta: 7
                },
                Entry {
                    account: 0,
                    delta: 5
                },
                Entry {
                    account: 1,
                    delta: 11
                }
            ],
            8
        ),
        Report {
            total: 60,
            lowest_balance: 7,
            first_overdraw: (-1)
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: (-3)
                },
                Entry {
                    account: 1,
                    delta: 7
                },
                Entry {
                    account: 1,
                    delta: 7
                },
                Entry {
                    account: 3,
                    delta: (-2)
                },
                Entry {
                    account: 4,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: 4
                },
                Entry {
                    account: 2,
                    delta: 8
                },
                Entry {
                    account: 1,
                    delta: (-2)
                }
            ],
            1
        ),
        Report {
            total: 23,
            lowest_balance: (-2),
            first_overdraw: 0
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 2,
                    delta: 10
                },
                Entry {
                    account: 2,
                    delta: 11
                },
                Entry {
                    account: 4,
                    delta: 2
                },
                Entry {
                    account: 3,
                    delta: 1
                },
                Entry {
                    account: 2,
                    delta: 1
                },
                Entry {
                    account: 2,
                    delta: 4
                },
                Entry {
                    account: 0,
                    delta: 2
                },
                Entry {
                    account: 3,
                    delta: (-3)
                },
                Entry {
                    account: 4,
                    delta: 2
                }
            ],
            2
        ),
        Report {
            total: 38,
            lowest_balance: 0,
            first_overdraw: (-1)
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: 7
                },
                Entry {
                    account: 3,
                    delta: (-3)
                },
                Entry {
                    account: 1,
                    delta: 5
                },
                Entry {
                    account: 0,
                    delta: 10
                },
                Entry {
                    account: 1,
                    delta: 8
                },
                Entry {
                    account: 3,
                    delta: 6
                },
                Entry {
                    account: 0,
                    delta: 2
                },
                Entry {
                    account: 4,
                    delta: 5
                },
                Entry {
                    account: 2,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: 1
                }
            ],
            3
        ),
        Report {
            total: 55,
            lowest_balance: 0,
            first_overdraw: (-1)
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 2,
                    delta: 5
                },
                Entry {
                    account: 4,
                    delta: 3
                },
                Entry {
                    account: 3,
                    delta: 11
                },
                Entry {
                    account: 1,
                    delta: 8
                },
                Entry {
                    account: 3,
                    delta: 4
                },
                Entry {
                    account: 0,
                    delta: 1
                },
                Entry {
                    account: 4,
                    delta: 3
                },
                Entry {
                    account: 1,
                    delta: 5
                },
                Entry {
                    account: 4,
                    delta: 6
                },
                Entry {
                    account: 1,
                    delta: 9
                },
                Entry {
                    account: 4,
                    delta: 7
                }
            ],
            4
        ),
        Report {
            total: 82,
            lowest_balance: 5,
            first_overdraw: (-1)
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 1,
                    delta: 3
                },
                Entry {
                    account: 1,
                    delta: 7
                },
                Entry {
                    account: 3,
                    delta: 4
                },
                Entry {
                    account: 2,
                    delta: 6
                },
                Entry {
                    account: 2,
                    delta: 11
                },
                Entry {
                    account: 1,
                    delta: 0
                },
                Entry {
                    account: 0,
                    delta: 2
                },
                Entry {
                    account: 1,
                    delta: 10
                },
                Entry {
                    account: 1,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: (-1)
                },
                Entry {
                    account: 3,
                    delta: 11
                },
                Entry {
                    account: 3,
                    delta: (-3)
                }
            ],
            5
        ),
        Report {
            total: 69,
            lowest_balance: 6,
            first_overdraw: (-1)
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            total: 0,
            lowest_balance: 6,
            first_overdraw: (-1)
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                account: 3,
                delta: (-1)
            }],
            7
        ),
        Report {
            total: 6,
            lowest_balance: 6,
            first_overdraw: (-1)
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: 4
                },
                Entry {
                    account: 4,
                    delta: 8
                }
            ],
            8
        ),
        Report {
            total: 28,
            lowest_balance: 12,
            first_overdraw: (-1)
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 3,
                    delta: 7
                },
                Entry {
                    account: 0,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: 3
                }
            ],
            1
        ),
        Report {
            total: 11,
            lowest_balance: 0,
            first_overdraw: (-1)
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 1,
                    delta: 11
                },
                Entry {
                    account: 0,
                    delta: 5
                },
                Entry {
                    account: 3,
                    delta: 4
                },
                Entry {
                    account: 0,
                    delta: 4
                }
            ],
            2
        ),
        Report {
            total: 30,
            lowest_balance: 6,
            first_overdraw: (-1)
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 1,
                    delta: 1
                },
                Entry {
                    account: 2,
                    delta: 5
                },
                Entry {
                    account: 4,
                    delta: (-3)
                },
                Entry {
                    account: 2,
                    delta: 5
                },
                Entry {
                    account: 1,
                    delta: (-3)
                }
            ],
            3
        ),
        Report {
            total: 14,
            lowest_balance: 0,
            first_overdraw: (-1)
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: 4
                },
                Entry {
                    account: 2,
                    delta: 1
                },
                Entry {
                    account: 3,
                    delta: 10
                },
                Entry {
                    account: 4,
                    delta: (-3)
                },
                Entry {
                    account: 2,
                    delta: 4
                },
                Entry {
                    account: 1,
                    delta: 4
                }
            ],
            4
        ),
        Report {
            total: 40,
            lowest_balance: 1,
            first_overdraw: (-1)
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 2,
                    delta: 1
                },
                Entry {
                    account: 0,
                    delta: 6
                },
                Entry {
                    account: 1,
                    delta: 6
                },
                Entry {
                    account: 0,
                    delta: 0
                },
                Entry {
                    account: 1,
                    delta: 0
                },
                Entry {
                    account: 2,
                    delta: 11
                },
                Entry {
                    account: 4,
                    delta: (-1)
                }
            ],
            5
        ),
        Report {
            total: 43,
            lowest_balance: 4,
            first_overdraw: (-1)
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 4,
                    delta: 6
                },
                Entry {
                    account: 4,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: (-3)
                },
                Entry {
                    account: 2,
                    delta: 9
                },
                Entry {
                    account: 0,
                    delta: (-1)
                },
                Entry {
                    account: 0,
                    delta: 2
                },
                Entry {
                    account: 1,
                    delta: (-2)
                },
                Entry {
                    account: 1,
                    delta: (-1)
                }
            ],
            6
        ),
        Report {
            total: 33,
            lowest_balance: 2,
            first_overdraw: (-1)
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: 3
                },
                Entry {
                    account: 1,
                    delta: 6
                }
            ],
            5
        ),
        Report {
            total: 19,
            lowest_balance: 8,
            first_overdraw: (-1)
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: (-5)
                },
                Entry {
                    account: 0,
                    delta: 1
                }
            ],
            5
        ),
        Report {
            total: 1,
            lowest_balance: 0,
            first_overdraw: (-1)
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 0,
                    delta: (-6)
                },
                Entry {
                    account: 1,
                    delta: (-8)
                }
            ],
            5
        ),
        Report {
            total: (-4),
            lowest_balance: (-3),
            first_overdraw: 0
        },
        "fixture 26"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    account: 2,
                    delta: (-7)
                },
                Entry {
                    account: 2,
                    delta: 9
                },
                Entry {
                    account: 3,
                    delta: (-1)
                }
            ],
            5
        ),
        Report {
            total: 11,
            lowest_balance: (-2),
            first_overdraw: 0
        },
        "fixture 27"
    );
}
