include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            allowed: 0,
            denied: 0,
            defaulted: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            allowed: 0,
            denied: 5,
            defaulted: 5
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            allowed: 0,
            denied: 1,
            defaulted: 1
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                principal: 3,
                allowed: 1,
                specificity: 1
            }],
            2
        ),
        Report {
            allowed: 0,
            denied: 2,
            defaulted: 2
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 0
                }
            ],
            3
        ),
        Report {
            allowed: 0,
            denied: 3,
            defaulted: 3
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 2
                }
            ],
            4
        ),
        Report {
            allowed: 2,
            denied: 2,
            defaulted: 2
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 0
                }
            ],
            5
        ),
        Report {
            allowed: 1,
            denied: 4,
            defaulted: 1
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 0
                }
            ],
            6
        ),
        Report {
            allowed: 2,
            denied: 4,
            defaulted: 3
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                }
            ],
            7
        ),
        Report {
            allowed: 2,
            denied: 5,
            defaulted: 4
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 1
                }
            ],
            8
        ),
        Report {
            allowed: 2,
            denied: 6,
            defaulted: 3
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 0
                }
            ],
            1
        ),
        Report {
            allowed: 1,
            denied: 0,
            defaulted: 0
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 0
                }
            ],
            2
        ),
        Report {
            allowed: 2,
            denied: 0,
            defaulted: 0
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 3
                }
            ],
            3
        ),
        Report {
            allowed: 1,
            denied: 2,
            defaulted: 1
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 0
                }
            ],
            4
        ),
        Report {
            allowed: 2,
            denied: 2,
            defaulted: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 1
                }
            ],
            5
        ),
        Report {
            allowed: 2,
            denied: 3,
            defaulted: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            allowed: 0,
            denied: 6,
            defaulted: 6
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                principal: 0,
                allowed: 0,
                specificity: 0
            }],
            7
        ),
        Report {
            allowed: 0,
            denied: 7,
            defaulted: 6
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 0
                }
            ],
            8
        ),
        Report {
            allowed: 1,
            denied: 7,
            defaulted: 6
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 2
                }
            ],
            1
        ),
        Report {
            allowed: 1,
            denied: 0,
            defaulted: 0
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 0
                }
            ],
            2
        ),
        Report {
            allowed: 2,
            denied: 0,
            defaulted: 0
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 0
                }
            ],
            3
        ),
        Report {
            allowed: 1,
            denied: 2,
            defaulted: 1
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 3
                }
            ],
            4
        ),
        Report {
            allowed: 1,
            denied: 3,
            defaulted: 2
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 3
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 2,
                    allowed: 0,
                    specificity: 3
                }
            ],
            5
        ),
        Report {
            allowed: 1,
            denied: 4,
            defaulted: 1
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 2,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 3,
                    allowed: 0,
                    specificity: 2
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 0
                },
                Entry {
                    principal: 4,
                    allowed: 0,
                    specificity: 2
                }
            ],
            6
        ),
        Report {
            allowed: 1,
            denied: 5,
            defaulted: 3
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 1
                }
            ],
            1
        ),
        Report {
            allowed: 1,
            denied: 0,
            defaulted: 0
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 4,
                    allowed: 1,
                    specificity: 8
                }
            ],
            2
        ),
        Report {
            allowed: 1,
            denied: 1,
            defaulted: 1
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    principal: 0,
                    allowed: 1,
                    specificity: 2
                },
                Entry {
                    principal: 0,
                    allowed: 0,
                    specificity: 1
                },
                Entry {
                    principal: 1,
                    allowed: 1,
                    specificity: 3
                },
                Entry {
                    principal: 1,
                    allowed: 0,
                    specificity: 3
                }
            ],
            3
        ),
        Report {
            allowed: 1,
            denied: 2,
            defaulted: 1
        },
        "fixture 26"
    );
}
