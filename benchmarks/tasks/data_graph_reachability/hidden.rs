include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            reachable: 0,
            unreachable: 0,
            reachable_edge_count: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            reachable: 1,
            unreachable: 4,
            reachable_edge_count: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            reachable: 1,
            unreachable: 0,
            reachable_edge_count: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                source: 1,
                target: 1
            }],
            2
        ),
        Report {
            reachable: 1,
            unreachable: 1,
            reachable_edge_count: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 1
                },
                Entry {
                    source: 2,
                    target: 2
                }
            ],
            3
        ),
        Report {
            reachable: 1,
            unreachable: 2,
            reachable_edge_count: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 3,
                    target: 1
                }
            ],
            4
        ),
        Report {
            reachable: 1,
            unreachable: 3,
            reachable_edge_count: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 2
                },
                Entry {
                    source: 1,
                    target: 3
                },
                Entry {
                    source: 1,
                    target: 3
                },
                Entry {
                    source: 2,
                    target: 0
                }
            ],
            5
        ),
        Report {
            reachable: 1,
            unreachable: 4,
            reachable_edge_count: 0
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 4,
                    target: 4
                },
                Entry {
                    source: 5,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 3
                },
                Entry {
                    source: 5,
                    target: 1
                }
            ],
            6
        ),
        Report {
            reachable: 3,
            unreachable: 3,
            reachable_edge_count: 2
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 6,
                    target: 4
                },
                Entry {
                    source: 3,
                    target: 5
                },
                Entry {
                    source: 2,
                    target: 3
                },
                Entry {
                    source: 6,
                    target: 5
                },
                Entry {
                    source: 4,
                    target: 4
                },
                Entry {
                    source: 4,
                    target: 0
                }
            ],
            7
        ),
        Report {
            reachable: 1,
            unreachable: 6,
            reachable_edge_count: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 4,
                    target: 1
                },
                Entry {
                    source: 2,
                    target: 0
                },
                Entry {
                    source: 6,
                    target: 6
                },
                Entry {
                    source: 6,
                    target: 0
                },
                Entry {
                    source: 7,
                    target: 2
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 3,
                    target: 6
                }
            ],
            8
        ),
        Report {
            reachable: 1,
            unreachable: 7,
            reachable_edge_count: 1
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                }
            ],
            1
        ),
        Report {
            reachable: 1,
            unreachable: 0,
            reachable_edge_count: 8
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 1
                }
            ],
            2
        ),
        Report {
            reachable: 2,
            unreachable: 0,
            reachable_edge_count: 9
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 2,
                    target: 2
                },
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 2
                },
                Entry {
                    source: 2,
                    target: 1
                },
                Entry {
                    source: 2,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 2
                },
                Entry {
                    source: 1,
                    target: 1
                }
            ],
            3
        ),
        Report {
            reachable: 3,
            unreachable: 0,
            reachable_edge_count: 10
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 2
                },
                Entry {
                    source: 3,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 3
                },
                Entry {
                    source: 2,
                    target: 2
                },
                Entry {
                    source: 2,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 2
                },
                Entry {
                    source: 1,
                    target: 2
                },
                Entry {
                    source: 1,
                    target: 0
                }
            ],
            4
        ),
        Report {
            reachable: 4,
            unreachable: 0,
            reachable_edge_count: 11
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 3
                },
                Entry {
                    source: 3,
                    target: 0
                },
                Entry {
                    source: 3,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 3
                },
                Entry {
                    source: 4,
                    target: 3
                },
                Entry {
                    source: 3,
                    target: 2
                },
                Entry {
                    source: 1,
                    target: 3
                },
                Entry {
                    source: 3,
                    target: 3
                },
                Entry {
                    source: 2,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 3
                },
                Entry {
                    source: 2,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 1
                }
            ],
            5
        ),
        Report {
            reachable: 4,
            unreachable: 1,
            reachable_edge_count: 11
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            reachable: 1,
            unreachable: 5,
            reachable_edge_count: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                source: 4,
                target: 6
            }],
            7
        ),
        Report {
            reachable: 1,
            unreachable: 6,
            reachable_edge_count: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 5,
                    target: 6
                },
                Entry {
                    source: 0,
                    target: 4
                }
            ],
            8
        ),
        Report {
            reachable: 2,
            unreachable: 6,
            reachable_edge_count: 1
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                }
            ],
            1
        ),
        Report {
            reachable: 1,
            unreachable: 0,
            reachable_edge_count: 3
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 1
                }
            ],
            2
        ),
        Report {
            reachable: 1,
            unreachable: 1,
            reachable_edge_count: 1
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 0
                },
                Entry {
                    source: 2,
                    target: 2
                },
                Entry {
                    source: 2,
                    target: 2
                },
                Entry {
                    source: 2,
                    target: 0
                },
                Entry {
                    source: 2,
                    target: 0
                }
            ],
            3
        ),
        Report {
            reachable: 1,
            unreachable: 2,
            reachable_edge_count: 0
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 2
                },
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 3,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 0,
                    target: 0
                },
                Entry {
                    source: 2,
                    target: 0
                }
            ],
            4
        ),
        Report {
            reachable: 3,
            unreachable: 1,
            reachable_edge_count: 5
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 1
                },
                Entry {
                    source: 2,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 2
                },
                Entry {
                    source: 4,
                    target: 3
                },
                Entry {
                    source: 1,
                    target: 4
                },
                Entry {
                    source: 4,
                    target: 3
                }
            ],
            5
        ),
        Report {
            reachable: 1,
            unreachable: 4,
            reachable_edge_count: 0
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 4
                },
                Entry {
                    source: 0,
                    target: 4
                },
                Entry {
                    source: 3,
                    target: 0
                },
                Entry {
                    source: 3,
                    target: 0
                },
                Entry {
                    source: 1,
                    target: 5
                },
                Entry {
                    source: 0,
                    target: 3
                },
                Entry {
                    source: 5,
                    target: 3
                },
                Entry {
                    source: 0,
                    target: 3
                }
            ],
            6
        ),
        Report {
            reachable: 3,
            unreachable: 3,
            reachable_edge_count: 5
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 2,
                    target: 3
                },
                Entry {
                    source: 1,
                    target: 2
                },
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 3,
                    target: 1
                },
                Entry {
                    source: 0,
                    target: 1
                }
            ],
            5
        ),
        Report {
            reachable: 4,
            unreachable: 1,
            reachable_edge_count: 5
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    source: 0,
                    target: 1
                },
                Entry {
                    source: 1,
                    target: 0
                },
                Entry {
                    source: 3,
                    target: 4
                },
                Entry {
                    source: 1,
                    target: 1
                }
            ],
            5
        ),
        Report {
            reachable: 2,
            unreachable: 3,
            reachable_edge_count: 3
        },
        "fixture 25"
    );
}
