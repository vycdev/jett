include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            restock_units: 0,
            at_risk: 0,
            worst_shortfall: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            restock_units: 0,
            at_risk: 0,
            worst_shortfall: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            restock_units: 0,
            at_risk: 0,
            worst_shortfall: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                stock: 8,
                daily_demand: 3
            }],
            2
        ),
        Report {
            restock_units: 0,
            at_risk: 0,
            worst_shortfall: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 6,
                    daily_demand: 3
                },
                Entry {
                    stock: 7,
                    daily_demand: 5
                }
            ],
            3
        ),
        Report {
            restock_units: 11,
            at_risk: 2,
            worst_shortfall: 8
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 11,
                    daily_demand: 1
                },
                Entry {
                    stock: 0,
                    daily_demand: 6
                },
                Entry {
                    stock: (-2),
                    daily_demand: 5
                }
            ],
            4
        ),
        Report {
            restock_units: 46,
            at_risk: 2,
            worst_shortfall: 24
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 10,
                    daily_demand: 2
                },
                Entry {
                    stock: 2,
                    daily_demand: 3
                },
                Entry {
                    stock: 0,
                    daily_demand: 4
                },
                Entry {
                    stock: 11,
                    daily_demand: 2
                }
            ],
            5
        ),
        Report {
            restock_units: 33,
            at_risk: 2,
            worst_shortfall: 20
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 10,
                    daily_demand: 3
                },
                Entry {
                    stock: 5,
                    daily_demand: 5
                },
                Entry {
                    stock: 2,
                    daily_demand: 6
                },
                Entry {
                    stock: 3,
                    daily_demand: 1
                },
                Entry {
                    stock: 1,
                    daily_demand: 1
                }
            ],
            6
        ),
        Report {
            restock_units: 75,
            at_risk: 5,
            worst_shortfall: 34
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 5,
                    daily_demand: 6
                },
                Entry {
                    stock: (-2),
                    daily_demand: 5
                },
                Entry {
                    stock: 11,
                    daily_demand: 6
                },
                Entry {
                    stock: 2,
                    daily_demand: 4
                },
                Entry {
                    stock: 10,
                    daily_demand: 6
                },
                Entry {
                    stock: 6,
                    daily_demand: 5
                }
            ],
            7
        ),
        Report {
            restock_units: 192,
            at_risk: 6,
            worst_shortfall: 37
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 5,
                    daily_demand: 1
                },
                Entry {
                    stock: 1,
                    daily_demand: 1
                },
                Entry {
                    stock: (-1),
                    daily_demand: 1
                },
                Entry {
                    stock: 8,
                    daily_demand: 4
                },
                Entry {
                    stock: 7,
                    daily_demand: 4
                },
                Entry {
                    stock: 5,
                    daily_demand: 4
                },
                Entry {
                    stock: 11,
                    daily_demand: 3
                }
            ],
            8
        ),
        Report {
            restock_units: 108,
            at_risk: 7,
            worst_shortfall: 27
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: (-3),
                    daily_demand: 3
                },
                Entry {
                    stock: 7,
                    daily_demand: 4
                },
                Entry {
                    stock: 7,
                    daily_demand: 1
                },
                Entry {
                    stock: (-2),
                    daily_demand: 3
                },
                Entry {
                    stock: (-1),
                    daily_demand: 4
                },
                Entry {
                    stock: 4,
                    daily_demand: 5
                },
                Entry {
                    stock: 8,
                    daily_demand: 4
                },
                Entry {
                    stock: (-2),
                    daily_demand: 6
                }
            ],
            1
        ),
        Report {
            restock_units: 25,
            at_risk: 5,
            worst_shortfall: 8
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 10,
                    daily_demand: 2
                },
                Entry {
                    stock: 11,
                    daily_demand: 2
                },
                Entry {
                    stock: 2,
                    daily_demand: 2
                },
                Entry {
                    stock: 1,
                    daily_demand: 2
                },
                Entry {
                    stock: 1,
                    daily_demand: 5
                },
                Entry {
                    stock: 4,
                    daily_demand: 2
                },
                Entry {
                    stock: 2,
                    daily_demand: 1
                },
                Entry {
                    stock: (-3),
                    daily_demand: 4
                },
                Entry {
                    stock: 2,
                    daily_demand: 2
                }
            ],
            2
        ),
        Report {
            restock_units: 27,
            at_risk: 5,
            worst_shortfall: 11
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 7,
                    daily_demand: 1
                },
                Entry {
                    stock: (-3),
                    daily_demand: 1
                },
                Entry {
                    stock: 5,
                    daily_demand: 6
                },
                Entry {
                    stock: 10,
                    daily_demand: 1
                },
                Entry {
                    stock: 8,
                    daily_demand: 3
                },
                Entry {
                    stock: 6,
                    daily_demand: 3
                },
                Entry {
                    stock: 2,
                    daily_demand: 3
                },
                Entry {
                    stock: 5,
                    daily_demand: 3
                },
                Entry {
                    stock: (-1),
                    daily_demand: 2
                },
                Entry {
                    stock: 1,
                    daily_demand: 2
                }
            ],
            3
        ),
        Report {
            restock_units: 46,
            at_risk: 8,
            worst_shortfall: 13
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 5,
                    daily_demand: 3
                },
                Entry {
                    stock: 3,
                    daily_demand: 6
                },
                Entry {
                    stock: 11,
                    daily_demand: 1
                },
                Entry {
                    stock: 8,
                    daily_demand: 6
                },
                Entry {
                    stock: 4,
                    daily_demand: 2
                },
                Entry {
                    stock: 1,
                    daily_demand: 1
                },
                Entry {
                    stock: 3,
                    daily_demand: 2
                },
                Entry {
                    stock: 5,
                    daily_demand: 6
                },
                Entry {
                    stock: 6,
                    daily_demand: 5
                },
                Entry {
                    stock: 9,
                    daily_demand: 5
                },
                Entry {
                    stock: 7,
                    daily_demand: 2
                }
            ],
            4
        ),
        Report {
            restock_units: 101,
            at_risk: 10,
            worst_shortfall: 21
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 3,
                    daily_demand: 2
                },
                Entry {
                    stock: 7,
                    daily_demand: 2
                },
                Entry {
                    stock: 4,
                    daily_demand: 6
                },
                Entry {
                    stock: 6,
                    daily_demand: 3
                },
                Entry {
                    stock: 11,
                    daily_demand: 3
                },
                Entry {
                    stock: 0,
                    daily_demand: 5
                },
                Entry {
                    stock: 2,
                    daily_demand: 2
                },
                Entry {
                    stock: 10,
                    daily_demand: 3
                },
                Entry {
                    stock: (-1),
                    daily_demand: 5
                },
                Entry {
                    stock: (-1),
                    daily_demand: 1
                },
                Entry {
                    stock: 11,
                    daily_demand: 6
                },
                Entry {
                    stock: (-3),
                    daily_demand: 4
                }
            ],
            5
        ),
        Report {
            restock_units: 161,
            at_risk: 12,
            worst_shortfall: 26
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            restock_units: 0,
            at_risk: 0,
            worst_shortfall: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                stock: (-1),
                daily_demand: 5
            }],
            7
        ),
        Report {
            restock_units: 36,
            at_risk: 1,
            worst_shortfall: 36
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 4,
                    daily_demand: 1
                },
                Entry {
                    stock: 8,
                    daily_demand: 4
                }
            ],
            8
        ),
        Report {
            restock_units: 28,
            at_risk: 2,
            worst_shortfall: 24
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 7,
                    daily_demand: 3
                },
                Entry {
                    stock: (-1),
                    daily_demand: 4
                },
                Entry {
                    stock: 3,
                    daily_demand: 4
                }
            ],
            1
        ),
        Report {
            restock_units: 6,
            at_risk: 2,
            worst_shortfall: 5
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 11,
                    daily_demand: 3
                },
                Entry {
                    stock: 5,
                    daily_demand: 2
                },
                Entry {
                    stock: 4,
                    daily_demand: 3
                },
                Entry {
                    stock: 4,
                    daily_demand: 2
                }
            ],
            2
        ),
        Report {
            restock_units: 2,
            at_risk: 1,
            worst_shortfall: 2
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 1,
                    daily_demand: 5
                },
                Entry {
                    stock: 5,
                    daily_demand: 4
                },
                Entry {
                    stock: (-3),
                    daily_demand: 3
                },
                Entry {
                    stock: 5,
                    daily_demand: 5
                },
                Entry {
                    stock: (-3),
                    daily_demand: 6
                }
            ],
            3
        ),
        Report {
            restock_units: 64,
            at_risk: 5,
            worst_shortfall: 21
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 4,
                    daily_demand: 4
                },
                Entry {
                    stock: 1,
                    daily_demand: 6
                },
                Entry {
                    stock: 10,
                    daily_demand: 1
                },
                Entry {
                    stock: (-3),
                    daily_demand: 1
                },
                Entry {
                    stock: 4,
                    daily_demand: 1
                },
                Entry {
                    stock: 4,
                    daily_demand: 6
                }
            ],
            4
        ),
        Report {
            restock_units: 62,
            at_risk: 4,
            worst_shortfall: 23
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 1,
                    daily_demand: 6
                },
                Entry {
                    stock: 6,
                    daily_demand: 5
                },
                Entry {
                    stock: 6,
                    daily_demand: 5
                },
                Entry {
                    stock: 0,
                    daily_demand: 5
                },
                Entry {
                    stock: 0,
                    daily_demand: 2
                },
                Entry {
                    stock: 11,
                    daily_demand: 1
                },
                Entry {
                    stock: (-1),
                    daily_demand: 4
                }
            ],
            5
        ),
        Report {
            restock_units: 123,
            at_risk: 6,
            worst_shortfall: 29
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 6,
                    daily_demand: 1
                },
                Entry {
                    stock: (-1),
                    daily_demand: 1
                },
                Entry {
                    stock: (-3),
                    daily_demand: 4
                },
                Entry {
                    stock: 9,
                    daily_demand: 5
                },
                Entry {
                    stock: (-1),
                    daily_demand: 2
                },
                Entry {
                    stock: 2,
                    daily_demand: 2
                },
                Entry {
                    stock: (-2),
                    daily_demand: 3
                },
                Entry {
                    stock: (-1),
                    daily_demand: 6
                }
            ],
            6
        ),
        Report {
            restock_units: 135,
            at_risk: 7,
            worst_shortfall: 37
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: (-5),
                    daily_demand: 3
                },
                Entry {
                    stock: 0,
                    daily_demand: 3
                }
            ],
            0
        ),
        Report {
            restock_units: 5,
            at_risk: 1,
            worst_shortfall: 5
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 6,
                    daily_demand: 3
                },
                Entry {
                    stock: 5,
                    daily_demand: 3
                }
            ],
            2
        ),
        Report {
            restock_units: 1,
            at_risk: 1,
            worst_shortfall: 1
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    stock: 5,
                    daily_demand: 3
                },
                Entry {
                    stock: 8,
                    daily_demand: 2
                },
                Entry {
                    stock: (-2),
                    daily_demand: 1
                }
            ],
            2
        ),
        Report {
            restock_units: 5,
            at_risk: 2,
            worst_shortfall: 4
        },
        "fixture 26"
    );
}
