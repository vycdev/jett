include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                node: 1,
                parent: 0,
                weight: 3
            }],
            2
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 10
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 5
                }
            ],
            3
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 3,
                    parent: 0,
                    weight: 8
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-8)
                },
                Entry {
                    node: 2,
                    parent: 0,
                    weight: (-6)
                }
            ],
            4
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 3,
                    parent: 0,
                    weight: 6
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 3
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 10
                },
                Entry {
                    node: 4,
                    parent: 0,
                    weight: (-2)
                }
            ],
            5
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 5,
                    parent: 4,
                    weight: (-1)
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: (-7)
                },
                Entry {
                    node: 3,
                    parent: 0,
                    weight: 0
                },
                Entry {
                    node: 4,
                    parent: 0,
                    weight: 5
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-4)
                }
            ],
            6
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 5,
                    parent: 2,
                    weight: (-5)
                },
                Entry {
                    node: 3,
                    parent: 2,
                    weight: (-2)
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 11
                },
                Entry {
                    node: 4,
                    parent: 0,
                    weight: (-1)
                },
                Entry {
                    node: 6,
                    parent: 3,
                    weight: (-4)
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-6)
                }
            ],
            7
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 6,
                    parent: 1,
                    weight: (-7)
                },
                Entry {
                    node: 7,
                    parent: 6,
                    weight: 7
                },
                Entry {
                    node: 4,
                    parent: 0,
                    weight: 1
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: (-3)
                },
                Entry {
                    node: 5,
                    parent: 1,
                    weight: 5
                },
                Entry {
                    node: 3,
                    parent: 1,
                    weight: (-8)
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 9
                }
            ],
            8
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[Entry {
                node: 1,
                parent: 0,
                weight: 11
            }],
            1
        ),
        Report {
            node_count: 1,
            total_weight: 11,
            leaf_count: 1
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 4
                },
                Entry {
                    node: 2,
                    parent: 0,
                    weight: (-5)
                }
            ],
            2
        ),
        Report {
            node_count: 1,
            total_weight: (-5),
            leaf_count: 1
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 0
                },
                Entry {
                    node: 3,
                    parent: 1,
                    weight: (-3)
                },
                Entry {
                    node: 2,
                    parent: 0,
                    weight: 10
                }
            ],
            3
        ),
        Report {
            node_count: 1,
            total_weight: (-3),
            leaf_count: 1
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 4,
                    parent: 1,
                    weight: (-7)
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 10
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 0
                },
                Entry {
                    node: 3,
                    parent: 1,
                    weight: 7
                }
            ],
            4
        ),
        Report {
            node_count: 1,
            total_weight: (-7),
            leaf_count: 1
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 2,
                    parent: 1,
                    weight: (-2)
                },
                Entry {
                    node: 3,
                    parent: 0,
                    weight: (-5)
                },
                Entry {
                    node: 4,
                    parent: 3,
                    weight: (-8)
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 5
                },
                Entry {
                    node: 5,
                    parent: 0,
                    weight: (-4)
                }
            ],
            5
        ),
        Report {
            node_count: 1,
            total_weight: (-4),
            leaf_count: 1
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                node: 1,
                parent: 0,
                weight: 4
            }],
            7
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-5)
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 0
                }
            ],
            8
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[Entry {
                node: 1,
                parent: 0,
                weight: (-3)
            }],
            1
        ),
        Report {
            node_count: 1,
            total_weight: (-3),
            leaf_count: 1
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-5)
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: (-2)
                }
            ],
            2
        ),
        Report {
            node_count: 1,
            total_weight: (-2),
            leaf_count: 1
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 8
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 4
                },
                Entry {
                    node: 3,
                    parent: 0,
                    weight: (-2)
                }
            ],
            3
        ),
        Report {
            node_count: 1,
            total_weight: (-2),
            leaf_count: 1
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 4,
                    parent: 3,
                    weight: (-1)
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-2)
                },
                Entry {
                    node: 3,
                    parent: 0,
                    weight: 8
                },
                Entry {
                    node: 2,
                    parent: 0,
                    weight: 0
                }
            ],
            4
        ),
        Report {
            node_count: 1,
            total_weight: (-1),
            leaf_count: 1
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 4,
                    parent: 3,
                    weight: 6
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: (-2)
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: (-2)
                },
                Entry {
                    node: 5,
                    parent: 2,
                    weight: 10
                },
                Entry {
                    node: 3,
                    parent: 0,
                    weight: (-3)
                }
            ],
            5
        ),
        Report {
            node_count: 1,
            total_weight: 10,
            leaf_count: 1
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 3,
                    parent: 0,
                    weight: (-1)
                },
                Entry {
                    node: 2,
                    parent: 0,
                    weight: 3
                },
                Entry {
                    node: 6,
                    parent: 0,
                    weight: (-4)
                },
                Entry {
                    node: 5,
                    parent: 1,
                    weight: 11
                },
                Entry {
                    node: 4,
                    parent: 2,
                    weight: (-3)
                },
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 9
                }
            ],
            6
        ),
        Report {
            node_count: 1,
            total_weight: (-4),
            leaf_count: 1
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 2
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 3
                },
                Entry {
                    node: 3,
                    parent: 1,
                    weight: 4
                },
                Entry {
                    node: 4,
                    parent: 2,
                    weight: (-5)
                },
                Entry {
                    node: 5,
                    parent: 0,
                    weight: 8
                }
            ],
            1
        ),
        Report {
            node_count: 4,
            total_weight: 4,
            leaf_count: 2
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[Entry {
                node: 1,
                parent: 0,
                weight: 2
            }],
            0
        ),
        Report {
            node_count: 0,
            total_weight: 0,
            leaf_count: 0
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 1,
                    parent: 0,
                    weight: 7
                },
                Entry {
                    node: 2,
                    parent: 1,
                    weight: 5
                },
                Entry {
                    node: 3,
                    parent: 1,
                    weight: 8
                },
                Entry {
                    node: 4,
                    parent: 2,
                    weight: (-2)
                }
            ],
            2
        ),
        Report {
            node_count: 2,
            total_weight: 3,
            leaf_count: 1
        },
        "fixture 26"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 30,
                    parent: 2,
                    weight: 7
                },
                Entry {
                    node: 12,
                    parent: 9,
                    weight: (-2)
                },
                Entry {
                    node: 2,
                    parent: 0,
                    weight: 3
                },
                Entry {
                    node: 9,
                    parent: 2,
                    weight: 5
                }
            ],
            9
        ),
        Report {
            node_count: 2,
            total_weight: 3,
            leaf_count: 1
        },
        "fixture 27"
    );
}
