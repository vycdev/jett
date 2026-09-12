include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            layers: 0,
            last_layer_count: 0,
            layer_sum: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            layers: 1,
            last_layer_count: 5,
            layer_sum: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            layers: 1,
            last_layer_count: 1,
            layer_sum: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                prerequisite: 0,
                dependent: 1
            }],
            2
        ),
        Report {
            layers: 2,
            last_layer_count: 1,
            layer_sum: 1
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                }
            ],
            3
        ),
        Report {
            layers: 2,
            last_layer_count: 1,
            layer_sum: 1
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                }
            ],
            4
        ),
        Report {
            layers: 2,
            last_layer_count: 2,
            layer_sum: 2
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                }
            ],
            5
        ),
        Report {
            layers: 3,
            last_layer_count: 1,
            layer_sum: 3
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 4
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 4,
                    dependent: 5
                },
                Entry {
                    prerequisite: 4,
                    dependent: 5
                }
            ],
            6
        ),
        Report {
            layers: 3,
            last_layer_count: 1,
            layer_sum: 4
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 6
                },
                Entry {
                    prerequisite: 4,
                    dependent: 5
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 4
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 3,
                    dependent: 5
                }
            ],
            7
        ),
        Report {
            layers: 3,
            last_layer_count: 1,
            layer_sum: 6
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 3,
                    dependent: 4
                },
                Entry {
                    prerequisite: 3,
                    dependent: 5
                },
                Entry {
                    prerequisite: 6,
                    dependent: 7
                },
                Entry {
                    prerequisite: 2,
                    dependent: 4
                },
                Entry {
                    prerequisite: 3,
                    dependent: 5
                },
                Entry {
                    prerequisite: 6,
                    dependent: 7
                },
                Entry {
                    prerequisite: 3,
                    dependent: 4
                }
            ],
            8
        ),
        Report {
            layers: 2,
            last_layer_count: 3,
            layer_sum: 3
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                }
            ],
            2
        ),
        Report {
            layers: 2,
            last_layer_count: 1,
            layer_sum: 1
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                }
            ],
            3
        ),
        Report {
            layers: 3,
            last_layer_count: 1,
            layer_sum: 3
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                }
            ],
            4
        ),
        Report {
            layers: 4,
            last_layer_count: 1,
            layer_sum: 6
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 1,
                    dependent: 4
                },
                Entry {
                    prerequisite: 3,
                    dependent: 4
                },
                Entry {
                    prerequisite: 2,
                    dependent: 4
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 4
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 3,
                    dependent: 4
                },
                Entry {
                    prerequisite: 3,
                    dependent: 4
                },
                Entry {
                    prerequisite: 0,
                    dependent: 4
                },
                Entry {
                    prerequisite: 3,
                    dependent: 4
                }
            ],
            5
        ),
        Report {
            layers: 4,
            last_layer_count: 1,
            layer_sum: 6
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            layers: 1,
            last_layer_count: 6,
            layer_sum: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(
            &[Entry {
                prerequisite: 2,
                dependent: 3
            }],
            7
        ),
        Report {
            layers: 2,
            last_layer_count: 1,
            layer_sum: 1
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 3,
                    dependent: 4
                },
                Entry {
                    prerequisite: 3,
                    dependent: 5
                }
            ],
            8
        ),
        Report {
            layers: 2,
            last_layer_count: 2,
            layer_sum: 2
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                }
            ],
            2
        ),
        Report {
            layers: 2,
            last_layer_count: 1,
            layer_sum: 1
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                }
            ],
            3
        ),
        Report {
            layers: 2,
            last_layer_count: 1,
            layer_sum: 1
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                }
            ],
            4
        ),
        Report {
            layers: 4,
            last_layer_count: 1,
            layer_sum: 6
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 4
                }
            ],
            5
        ),
        Report {
            layers: 4,
            last_layer_count: 1,
            layer_sum: 8
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 0,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 3
                },
                Entry {
                    prerequisite: 2,
                    dependent: 5
                },
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 3,
                    dependent: 5
                },
                Entry {
                    prerequisite: 2,
                    dependent: 5
                },
                Entry {
                    prerequisite: 0,
                    dependent: 5
                }
            ],
            6
        ),
        Report {
            layers: 4,
            last_layer_count: 1,
            layer_sum: 6
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 3,
                    dependent: 4
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 0,
                    dependent: 1
                },
                Entry {
                    prerequisite: 0,
                    dependent: 4
                }
            ],
            6
        ),
        Report {
            layers: 5,
            last_layer_count: 1,
            layer_sum: 10
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    prerequisite: 0,
                    dependent: 2
                },
                Entry {
                    prerequisite: 1,
                    dependent: 2
                },
                Entry {
                    prerequisite: 2,
                    dependent: 3
                },
                Entry {
                    prerequisite: 1,
                    dependent: 4
                }
            ],
            5
        ),
        Report {
            layers: 3,
            last_layer_count: 1,
            layer_sum: 4
        },
        "fixture 23"
    );
}
