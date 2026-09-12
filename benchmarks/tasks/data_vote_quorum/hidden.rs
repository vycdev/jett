include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                voter: 3,
                choice: 2,
                weight: 1
            }],
            2
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: 4
                },
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: (-2)
                }
            ],
            3
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 8
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 6
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: 3
                }
            ],
            4
        ),
        Report {
            yes_weight: 3,
            no_weight: 6,
            quorum_met: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 2,
                    choice: 2,
                    weight: 7
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: (-1)
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: 3
                },
                Entry {
                    voter: 0,
                    choice: 2,
                    weight: 9
                }
            ],
            5
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: (-2)
                },
                Entry {
                    voter: 1,
                    choice: 2,
                    weight: (-1)
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: (-1)
                },
                Entry {
                    voter: 4,
                    choice: 3,
                    weight: 9
                },
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: 5
                }
            ],
            6
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: 7
                },
                Entry {
                    voter: 4,
                    choice: 1,
                    weight: 6
                },
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 2
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: 0
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 9
                },
                Entry {
                    voter: 3,
                    choice: 3,
                    weight: 8
                }
            ],
            7
        ),
        Report {
            yes_weight: 6,
            no_weight: 9,
            quorum_met: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 6
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: 3
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 2
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: 0
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: (-1)
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 4
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: 7
                }
            ],
            8
        ),
        Report {
            yes_weight: 7,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 2,
                    choice: 3,
                    weight: 0
                },
                Entry {
                    voter: 0,
                    choice: 2,
                    weight: 1
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 7
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 4
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 2
                },
                Entry {
                    voter: 2,
                    choice: 2,
                    weight: 5
                },
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 3
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: (-2)
                }
            ],
            1
        ),
        Report {
            yes_weight: 0,
            no_weight: 3,
            quorum_met: 0
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 3,
                    choice: 2,
                    weight: 1
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 5
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 0
                },
                Entry {
                    voter: 4,
                    choice: 0,
                    weight: (-1)
                },
                Entry {
                    voter: 1,
                    choice: 2,
                    weight: 4
                },
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: 9
                },
                Entry {
                    voter: 0,
                    choice: 2,
                    weight: 2
                },
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: 3
                },
                Entry {
                    voter: 1,
                    choice: 1,
                    weight: (-1)
                }
            ],
            2
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 3
                },
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: 6
                },
                Entry {
                    voter: 3,
                    choice: 3,
                    weight: (-2)
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: 5
                },
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 2
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: 1
                },
                Entry {
                    voter: 1,
                    choice: 1,
                    weight: 7
                },
                Entry {
                    voter: 4,
                    choice: 1,
                    weight: 1
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: 0
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: 5
                }
            ],
            3
        ),
        Report {
            yes_weight: 4,
            no_weight: 0,
            quorum_met: 1
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 2,
                    choice: 2,
                    weight: 2
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 1
                },
                Entry {
                    voter: 4,
                    choice: 0,
                    weight: 3
                },
                Entry {
                    voter: 1,
                    choice: 1,
                    weight: 2
                },
                Entry {
                    voter: 1,
                    choice: 1,
                    weight: 7
                },
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: (-1)
                },
                Entry {
                    voter: 3,
                    choice: 3,
                    weight: (-2)
                },
                Entry {
                    voter: 3,
                    choice: 3,
                    weight: 0
                },
                Entry {
                    voter: 4,
                    choice: 0,
                    weight: 5
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: 4
                },
                Entry {
                    voter: 2,
                    choice: 0,
                    weight: 0
                }
            ],
            4
        ),
        Report {
            yes_weight: 7,
            no_weight: 5,
            quorum_met: 1
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 4
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: 3
                },
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 5
                },
                Entry {
                    voter: 3,
                    choice: 2,
                    weight: (-2)
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: 1
                },
                Entry {
                    voter: 2,
                    choice: 2,
                    weight: 6
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 2
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: (-2)
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: 4
                },
                Entry {
                    voter: 2,
                    choice: 2,
                    weight: 8
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 6
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 3
                }
            ],
            5
        ),
        Report {
            yes_weight: 0,
            no_weight: 9,
            quorum_met: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                voter: 3,
                choice: 0,
                weight: 1
            }],
            7
        ),
        Report {
            yes_weight: 0,
            no_weight: 1,
            quorum_met: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 3,
                    choice: 2,
                    weight: 2
                },
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 7
                }
            ],
            8
        ),
        Report {
            yes_weight: 7,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 4,
                    choice: 0,
                    weight: 1
                },
                Entry {
                    voter: 4,
                    choice: 1,
                    weight: 1
                },
                Entry {
                    voter: 1,
                    choice: 2,
                    weight: (-2)
                }
            ],
            1
        ),
        Report {
            yes_weight: 1,
            no_weight: 0,
            quorum_met: 1
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 4,
                    choice: 1,
                    weight: 5
                },
                Entry {
                    voter: 4,
                    choice: 0,
                    weight: 6
                },
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 8
                },
                Entry {
                    voter: 0,
                    choice: 0,
                    weight: 5
                }
            ],
            2
        ),
        Report {
            yes_weight: 0,
            no_weight: 19,
            quorum_met: 0
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 2,
                    choice: 0,
                    weight: 0
                },
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 3
                },
                Entry {
                    voter: 1,
                    choice: 1,
                    weight: (-1)
                },
                Entry {
                    voter: 2,
                    choice: 1,
                    weight: 0
                },
                Entry {
                    voter: 1,
                    choice: 2,
                    weight: 2
                }
            ],
            3
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 4,
                    choice: 3,
                    weight: (-2)
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: 6
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 2
                },
                Entry {
                    voter: 4,
                    choice: 1,
                    weight: (-2)
                },
                Entry {
                    voter: 4,
                    choice: 3,
                    weight: (-2)
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 0
                }
            ],
            4
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 5
                },
                Entry {
                    voter: 3,
                    choice: 2,
                    weight: (-1)
                },
                Entry {
                    voter: 3,
                    choice: 3,
                    weight: 5
                },
                Entry {
                    voter: 0,
                    choice: 3,
                    weight: (-2)
                },
                Entry {
                    voter: 4,
                    choice: 1,
                    weight: (-1)
                },
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 3
                },
                Entry {
                    voter: 2,
                    choice: 3,
                    weight: (-1)
                }
            ],
            5
        ),
        Report {
            yes_weight: 0,
            no_weight: 3,
            quorum_met: 0
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 2,
                    choice: 3,
                    weight: (-1)
                },
                Entry {
                    voter: 2,
                    choice: 0,
                    weight: (-1)
                },
                Entry {
                    voter: 2,
                    choice: 2,
                    weight: 5
                },
                Entry {
                    voter: 1,
                    choice: 3,
                    weight: 3
                },
                Entry {
                    voter: 4,
                    choice: 2,
                    weight: 6
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 1
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 1
                },
                Entry {
                    voter: 3,
                    choice: 0,
                    weight: 9
                }
            ],
            6
        ),
        Report {
            yes_weight: 0,
            no_weight: 9,
            quorum_met: 0
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 5
                },
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 0
                }
            ],
            1
        ),
        Report {
            yes_weight: 0,
            no_weight: 0,
            quorum_met: 0
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 0,
                    choice: 1,
                    weight: 3
                },
                Entry {
                    voter: 1,
                    choice: 0,
                    weight: 3
                }
            ],
            3
        ),
        Report {
            yes_weight: 3,
            no_weight: 3,
            quorum_met: 0
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[Entry {
                voter: 0,
                choice: 1,
                weight: 3
            }],
            3
        ),
        Report {
            yes_weight: 3,
            no_weight: 0,
            quorum_met: 1
        },
        "fixture 26"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    voter: 1,
                    choice: 1,
                    weight: 5
                },
                Entry {
                    voter: 2,
                    choice: 0,
                    weight: 3
                },
                Entry {
                    voter: 1,
                    choice: 2,
                    weight: 9
                },
                Entry {
                    voter: 3,
                    choice: 1,
                    weight: 4
                }
            ],
            4
        ),
        Report {
            yes_weight: 4,
            no_weight: 3,
            quorum_met: 1
        },
        "fixture 27"
    );
}
