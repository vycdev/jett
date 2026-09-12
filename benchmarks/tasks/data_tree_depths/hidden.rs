include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            roots: 0,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            node: 1,
            parent: 0,
            weight: 3
        }]),
        Report {
            roots: 1,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 1,
            max_depth: 1,
            weighted_depth: 5
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 3,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 3,
            max_depth: 1,
            weighted_depth: 10
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 3,
            max_depth: 1,
            weighted_depth: (-8)
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 2,
            max_depth: 3,
            weighted_depth: (-15)
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 2,
            max_depth: 2,
            weighted_depth: 1
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[Entry {
            node: 1,
            parent: 0,
            weight: 11
        }]),
        Report {
            roots: 1,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 2,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 2,
            max_depth: 1,
            weighted_depth: (-3)
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 1,
            max_depth: 1,
            weighted_depth: 10
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 3,
            max_depth: 1,
            weighted_depth: (-10)
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            node: 1,
            parent: 0,
            weight: 4
        }]),
        Report {
            roots: 1,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 1,
            max_depth: 1,
            weighted_depth: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[Entry {
            node: 1,
            parent: 0,
            weight: (-3)
        }]),
        Report {
            roots: 1,
            max_depth: 0,
            weighted_depth: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 1,
            max_depth: 1,
            weighted_depth: (-2)
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 2,
            max_depth: 1,
            weighted_depth: 4
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 3,
            max_depth: 1,
            weighted_depth: (-1)
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 2,
            max_depth: 2,
            weighted_depth: 24
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 4,
            max_depth: 1,
            weighted_depth: 8
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                node: 5,
                parent: 4,
                weight: (-1)
            },
            Entry {
                node: 4,
                parent: 3,
                weight: 2
            },
            Entry {
                node: 3,
                parent: 2,
                weight: 4
            },
            Entry {
                node: 2,
                parent: 1,
                weight: 8
            },
            Entry {
                node: 1,
                parent: 0,
                weight: 3
            }
        ]),
        Report {
            roots: 1,
            max_depth: 4,
            weighted_depth: 18
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                node: 4,
                parent: 2,
                weight: 3
            },
            Entry {
                node: 2,
                parent: 1,
                weight: 5
            },
            Entry {
                node: 1,
                parent: 0,
                weight: 9
            },
            Entry {
                node: 3,
                parent: 0,
                weight: 7
            }
        ]),
        Report {
            roots: 2,
            max_depth: 2,
            weighted_depth: 11
        },
        "fixture 22"
    );
    assert_eq!(
        solve(&[
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
        ]),
        Report {
            roots: 1,
            max_depth: 2,
            weighted_depth: 8
        },
        "fixture 23"
    );
}
