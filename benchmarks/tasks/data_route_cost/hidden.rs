include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            reachable: 0,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                source: 0,
                target: 1,
                cost: 2
            }],
            2
        ),
        Report {
            reachable: 2,
            cost_sum: 2,
            most_expensive: 2
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 2,
                    cost: 6
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 1
                }
            ],
            3
        ),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 3,
                    cost: 1
                },
                Entry {
                    source: 0,
                    target: 3,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 3
                }
            ],
            4
        ),
        Report {
            reachable: 3,
            cost_sum: 4,
            most_expensive: 3
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 3,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 4
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 5
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 4
                }
            ],
            5
        ),
        Report {
            reachable: 3,
            cost_sum: 12,
            most_expensive: 8
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 2,
                    cost: 3
                },
                Entry {
                    source: 0,
                    target: 4,
                    cost: 5
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 7
                },
                Entry {
                    source: 4,
                    target: 5,
                    cost: 6
                },
                Entry {
                    source: 4,
                    target: 5,
                    cost: 4
                }
            ],
            6
        ),
        Report {
            reachable: 4,
            cost_sum: 17,
            most_expensive: 9
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 6,
                    cost: 4
                },
                Entry {
                    source: 4,
                    target: 5,
                    cost: 5
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 3
                },
                Entry {
                    source: 0,
                    target: 4,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 6
                },
                Entry {
                    source: 3,
                    target: 5,
                    cost: 6
                }
            ],
            7
        ),
        Report {
            reachable: 6,
            cost_sum: 22,
            most_expensive: 7
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 3,
                    target: 4,
                    cost: 5
                },
                Entry {
                    source: 3,
                    target: 5,
                    cost: 3
                },
                Entry {
                    source: 6,
                    target: 7,
                    cost: 1
                },
                Entry {
                    source: 2,
                    target: 4,
                    cost: 6
                },
                Entry {
                    source: 3,
                    target: 5,
                    cost: 6
                },
                Entry {
                    source: 6,
                    target: 7,
                    cost: 7
                },
                Entry {
                    source: 3,
                    target: 4,
                    cost: 7
                }
            ],
            8
        ),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1,
                    cost: 7
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 7
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 7
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 6
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 7
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 3
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 4
                }
            ],
            2
        ),
        Report {
            reachable: 2,
            cost_sum: 3,
            most_expensive: 3
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1,
                    cost: 3
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 1
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 6
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 4
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 3
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 3
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 1
                }
            ],
            3
        ),
        Report {
            reachable: 3,
            cost_sum: 2,
            most_expensive: 1
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 2,
                    cost: 3
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 5
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 1
                },
                Entry {
                    source: 0,
                    target: 3,
                    cost: 6
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 3
                },
                Entry {
                    source: 0,
                    target: 3,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 5
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 7
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 7
                }
            ],
            4
        ),
        Report {
            reachable: 4,
            cost_sum: 17,
            most_expensive: 8
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 4,
                    cost: 2
                },
                Entry {
                    source: 3,
                    target: 4,
                    cost: 6
                },
                Entry {
                    source: 2,
                    target: 4,
                    cost: 3
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 4,
                    cost: 1
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 7
                },
                Entry {
                    source: 3,
                    target: 4,
                    cost: 1
                },
                Entry {
                    source: 3,
                    target: 4,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 4,
                    cost: 1
                },
                Entry {
                    source: 3,
                    target: 4,
                    cost: 6
                }
            ],
            5
        ),
        Report {
            reachable: 3,
            cost_sum: 3,
            most_expensive: 2
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(
            &[Entry {
                source: 2,
                target: 3,
                cost: 2
            }],
            7
        ),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 3,
                    target: 4,
                    cost: 4
                },
                Entry {
                    source: 3,
                    target: 5,
                    cost: 3
                }
            ],
            8
        ),
        Report {
            reachable: 1,
            cost_sum: 0,
            most_expensive: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 1
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 5
                }
            ],
            2
        ),
        Report {
            reachable: 2,
            cost_sum: 1,
            most_expensive: 1
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 2,
                    cost: 3
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 1
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 4
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 7
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 5
                }
            ],
            3
        ),
        Report {
            reachable: 2,
            cost_sum: 4,
            most_expensive: 4
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1,
                    cost: 3
                },
                Entry {
                    source: 1,
                    target: 2,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 3
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 5
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 5
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 2
                }
            ],
            4
        ),
        Report {
            reachable: 4,
            cost_sum: 14,
            most_expensive: 6
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 2,
                    cost: 2
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 6
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 5
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 1
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 4
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 4,
                    cost: 1
                }
            ],
            5
        ),
        Report {
            reachable: 5,
            cost_sum: 13,
            most_expensive: 5
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 3,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 3
                },
                Entry {
                    source: 2,
                    target: 5,
                    cost: 4
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 5
                },
                Entry {
                    source: 3,
                    target: 5,
                    cost: 1
                },
                Entry {
                    source: 2,
                    target: 5,
                    cost: 2
                },
                Entry {
                    source: 0,
                    target: 5,
                    cost: 5
                }
            ],
            6
        ),
        Report {
            reachable: 4,
            cost_sum: 10,
            most_expensive: 5
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1,
                    cost: 9
                },
                Entry {
                    source: 0,
                    target: 1,
                    cost: 3
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 5
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 1
                },
                Entry {
                    source: 0,
                    target: 3,
                    cost: 12
                }
            ],
            5
        ),
        Report {
            reachable: 3,
            cost_sum: 11,
            most_expensive: 8
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1,
                    cost: 8
                },
                Entry {
                    source: 0,
                    target: 2,
                    cost: 2
                },
                Entry {
                    source: 1,
                    target: 3,
                    cost: 1
                },
                Entry {
                    source: 2,
                    target: 3,
                    cost: 3
                }
            ],
            5
        ),
        Report {
            reachable: 4,
            cost_sum: 15,
            most_expensive: 8
        },
        "fixture 23"
    );
}
