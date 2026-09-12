include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            ready: 0,
            blocked: 0,
            total_cost: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            job: 1,
            prerequisite: 0,
            cost: 3
        }]),
        Report {
            ready: 1,
            blocked: 0,
            total_cost: 3
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 10
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 5
            }
        ]),
        Report {
            ready: 2,
            blocked: 0,
            total_cost: 15
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 3,
                prerequisite: 0,
                cost: 8
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-8)
            },
            Entry {
                job: 2,
                prerequisite: 0,
                cost: (-6)
            }
        ]),
        Report {
            ready: 3,
            blocked: 0,
            total_cost: (-6)
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 3,
                prerequisite: 0,
                cost: 6
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 3
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 10
            },
            Entry {
                job: 4,
                prerequisite: 0,
                cost: (-2)
            }
        ]),
        Report {
            ready: 4,
            blocked: 0,
            total_cost: 17
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 5,
                prerequisite: 4,
                cost: (-1)
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: (-7)
            },
            Entry {
                job: 3,
                prerequisite: 0,
                cost: 0
            },
            Entry {
                job: 4,
                prerequisite: 0,
                cost: 5
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-4)
            }
        ]),
        Report {
            ready: 5,
            blocked: 0,
            total_cost: (-7)
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 5,
                prerequisite: 2,
                cost: (-5)
            },
            Entry {
                job: 3,
                prerequisite: 2,
                cost: (-2)
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 11
            },
            Entry {
                job: 4,
                prerequisite: 0,
                cost: (-1)
            },
            Entry {
                job: 6,
                prerequisite: 3,
                cost: (-4)
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-6)
            }
        ]),
        Report {
            ready: 6,
            blocked: 0,
            total_cost: (-7)
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 6,
                prerequisite: 1,
                cost: (-7)
            },
            Entry {
                job: 7,
                prerequisite: 6,
                cost: 7
            },
            Entry {
                job: 4,
                prerequisite: 0,
                cost: 1
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: (-3)
            },
            Entry {
                job: 5,
                prerequisite: 1,
                cost: 5
            },
            Entry {
                job: 3,
                prerequisite: 1,
                cost: (-8)
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 9
            }
        ]),
        Report {
            ready: 7,
            blocked: 0,
            total_cost: 4
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[Entry {
            job: 1,
            prerequisite: 0,
            cost: 11
        }]),
        Report {
            ready: 1,
            blocked: 0,
            total_cost: 11
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 4
            },
            Entry {
                job: 2,
                prerequisite: 0,
                cost: (-5)
            }
        ]),
        Report {
            ready: 2,
            blocked: 0,
            total_cost: (-1)
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 0
            },
            Entry {
                job: 3,
                prerequisite: 1,
                cost: (-3)
            },
            Entry {
                job: 2,
                prerequisite: 0,
                cost: 10
            }
        ]),
        Report {
            ready: 3,
            blocked: 0,
            total_cost: 7
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 4,
                prerequisite: 1,
                cost: (-7)
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 10
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 0
            },
            Entry {
                job: 3,
                prerequisite: 1,
                cost: 7
            }
        ]),
        Report {
            ready: 4,
            blocked: 0,
            total_cost: 10
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 2,
                prerequisite: 1,
                cost: (-2)
            },
            Entry {
                job: 3,
                prerequisite: 0,
                cost: (-5)
            },
            Entry {
                job: 4,
                prerequisite: 3,
                cost: (-8)
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 5
            },
            Entry {
                job: 5,
                prerequisite: 0,
                cost: (-4)
            }
        ]),
        Report {
            ready: 5,
            blocked: 0,
            total_cost: (-14)
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            job: 1,
            prerequisite: 0,
            cost: 4
        }]),
        Report {
            ready: 1,
            blocked: 0,
            total_cost: 4
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-5)
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 0
            }
        ]),
        Report {
            ready: 2,
            blocked: 0,
            total_cost: (-5)
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[Entry {
            job: 1,
            prerequisite: 0,
            cost: (-3)
        }]),
        Report {
            ready: 1,
            blocked: 0,
            total_cost: (-3)
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-5)
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: (-2)
            }
        ]),
        Report {
            ready: 2,
            blocked: 0,
            total_cost: (-7)
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 8
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 4
            },
            Entry {
                job: 3,
                prerequisite: 0,
                cost: (-2)
            }
        ]),
        Report {
            ready: 3,
            blocked: 0,
            total_cost: 10
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 4,
                prerequisite: 3,
                cost: (-1)
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-2)
            },
            Entry {
                job: 3,
                prerequisite: 0,
                cost: 8
            },
            Entry {
                job: 2,
                prerequisite: 0,
                cost: 0
            }
        ]),
        Report {
            ready: 4,
            blocked: 0,
            total_cost: 5
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 4,
                prerequisite: 3,
                cost: 6
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-2)
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: (-2)
            },
            Entry {
                job: 5,
                prerequisite: 2,
                cost: 10
            },
            Entry {
                job: 3,
                prerequisite: 0,
                cost: (-3)
            }
        ]),
        Report {
            ready: 5,
            blocked: 0,
            total_cost: 9
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 3,
                prerequisite: 0,
                cost: (-1)
            },
            Entry {
                job: 2,
                prerequisite: 0,
                cost: 3
            },
            Entry {
                job: 6,
                prerequisite: 0,
                cost: (-4)
            },
            Entry {
                job: 5,
                prerequisite: 1,
                cost: 11
            },
            Entry {
                job: 4,
                prerequisite: 2,
                cost: (-3)
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 9
            }
        ]),
        Report {
            ready: 6,
            blocked: 0,
            total_cost: 15
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 9,
                prerequisite: 8,
                cost: 5
            },
            Entry {
                job: 3,
                prerequisite: 2,
                cost: 1
            },
            Entry {
                job: 2,
                prerequisite: 1,
                cost: 8
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: (-2)
            },
            Entry {
                job: 10,
                prerequisite: 9,
                cost: 3
            }
        ]),
        Report {
            ready: 3,
            blocked: 2,
            total_cost: 7
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                job: 3,
                prerequisite: 2,
                cost: 5
            },
            Entry {
                job: 1,
                prerequisite: 0,
                cost: 2
            },
            Entry {
                job: 5,
                prerequisite: 3,
                cost: 9
            }
        ]),
        Report {
            ready: 1,
            blocked: 2,
            total_cost: 2
        },
        "fixture 22"
    );
}
