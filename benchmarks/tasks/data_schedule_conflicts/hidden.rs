include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            start: 11,
            end: 15,
            resource_id: 3
        }]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 3,
                end: 8,
                resource_id: 3
            },
            Entry {
                start: 6,
                end: 12,
                resource_id: 5
            }
        ]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 10,
                end: 13,
                resource_id: 1
            },
            Entry {
                start: 0,
                end: 2,
                resource_id: 1
            },
            Entry {
                start: 1,
                end: 6,
                resource_id: 4
            }
        ]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 3,
                end: 6,
                resource_id: 3
            },
            Entry {
                start: 5,
                end: 10,
                resource_id: 2
            },
            Entry {
                start: 7,
                end: 8,
                resource_id: 2
            },
            Entry {
                start: 6,
                end: 13,
                resource_id: 3
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 1
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 0,
                end: 5,
                resource_id: 5
            },
            Entry {
                start: 9,
                end: 12,
                resource_id: 2
            },
            Entry {
                start: 6,
                end: 7,
                resource_id: 2
            },
            Entry {
                start: 4,
                end: 5,
                resource_id: 4
            },
            Entry {
                start: 8,
                end: 15,
                resource_id: 2
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 3
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 1,
                end: 8,
                resource_id: 5
            },
            Entry {
                start: 7,
                end: 13,
                resource_id: 5
            },
            Entry {
                start: 5,
                end: 9,
                resource_id: 1
            },
            Entry {
                start: 10,
                end: 14,
                resource_id: 5
            },
            Entry {
                start: 9,
                end: 11,
                resource_id: 5
            },
            Entry {
                start: 0,
                end: 2,
                resource_id: 3
            }
        ]),
        Report {
            conflict_pairs: 4,
            affected_bookings: 4,
            longest_overlap: 3
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 1,
                end: 5,
                resource_id: 2
            },
            Entry {
                start: 0,
                end: 1,
                resource_id: 4
            },
            Entry {
                start: 6,
                end: 12,
                resource_id: 4
            },
            Entry {
                start: 11,
                end: 12,
                resource_id: 5
            },
            Entry {
                start: 7,
                end: 14,
                resource_id: 2
            },
            Entry {
                start: 5,
                end: 12,
                resource_id: 1
            },
            Entry {
                start: 0,
                end: 3,
                resource_id: 2
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 2
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 10,
                end: 14,
                resource_id: 2
            },
            Entry {
                start: 10,
                end: 17,
                resource_id: 1
            },
            Entry {
                start: 7,
                end: 8,
                resource_id: 3
            },
            Entry {
                start: 9,
                end: 11,
                resource_id: 4
            },
            Entry {
                start: 11,
                end: 12,
                resource_id: 4
            },
            Entry {
                start: 9,
                end: 12,
                resource_id: 4
            },
            Entry {
                start: 2,
                end: 3,
                resource_id: 3
            },
            Entry {
                start: 3,
                end: 9,
                resource_id: 3
            }
        ]),
        Report {
            conflict_pairs: 3,
            affected_bookings: 5,
            longest_overlap: 2
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 2,
                end: 9,
                resource_id: 5
            },
            Entry {
                start: 5,
                end: 7,
                resource_id: 4
            },
            Entry {
                start: 4,
                end: 6,
                resource_id: 3
            },
            Entry {
                start: 4,
                end: 11,
                resource_id: 5
            },
            Entry {
                start: 5,
                end: 9,
                resource_id: 2
            },
            Entry {
                start: 10,
                end: 11,
                resource_id: 3
            },
            Entry {
                start: 0,
                end: 4,
                resource_id: 1
            },
            Entry {
                start: 6,
                end: 11,
                resource_id: 3
            },
            Entry {
                start: 3,
                end: 4,
                resource_id: 1
            }
        ]),
        Report {
            conflict_pairs: 3,
            affected_bookings: 6,
            longest_overlap: 5
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 7,
                end: 8,
                resource_id: 1
            },
            Entry {
                start: 2,
                end: 7,
                resource_id: 1
            },
            Entry {
                start: 1,
                end: 7,
                resource_id: 2
            },
            Entry {
                start: 11,
                end: 14,
                resource_id: 4
            },
            Entry {
                start: 9,
                end: 12,
                resource_id: 1
            },
            Entry {
                start: 5,
                end: 8,
                resource_id: 5
            },
            Entry {
                start: 8,
                end: 11,
                resource_id: 3
            },
            Entry {
                start: 2,
                end: 4,
                resource_id: 1
            },
            Entry {
                start: 4,
                end: 6,
                resource_id: 3
            },
            Entry {
                start: 8,
                end: 11,
                resource_id: 5
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 2
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 6,
                end: 12,
                resource_id: 4
            },
            Entry {
                start: 0,
                end: 2,
                resource_id: 4
            },
            Entry {
                start: 7,
                end: 9,
                resource_id: 1
            },
            Entry {
                start: 4,
                end: 5,
                resource_id: 5
            },
            Entry {
                start: 6,
                end: 8,
                resource_id: 2
            },
            Entry {
                start: 8,
                end: 14,
                resource_id: 5
            },
            Entry {
                start: 9,
                end: 14,
                resource_id: 2
            },
            Entry {
                start: 9,
                end: 14,
                resource_id: 2
            },
            Entry {
                start: 3,
                end: 7,
                resource_id: 2
            },
            Entry {
                start: 2,
                end: 8,
                resource_id: 2
            },
            Entry {
                start: 7,
                end: 11,
                resource_id: 3
            }
        ]),
        Report {
            conflict_pairs: 4,
            affected_bookings: 5,
            longest_overlap: 5
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 9,
                end: 12,
                resource_id: 3
            },
            Entry {
                start: 5,
                end: 12,
                resource_id: 2
            },
            Entry {
                start: 3,
                end: 8,
                resource_id: 1
            },
            Entry {
                start: 5,
                end: 7,
                resource_id: 2
            },
            Entry {
                start: 4,
                end: 6,
                resource_id: 2
            },
            Entry {
                start: 9,
                end: 10,
                resource_id: 2
            },
            Entry {
                start: 1,
                end: 8,
                resource_id: 4
            },
            Entry {
                start: 11,
                end: 15,
                resource_id: 1
            },
            Entry {
                start: 6,
                end: 10,
                resource_id: 2
            },
            Entry {
                start: 8,
                end: 9,
                resource_id: 4
            },
            Entry {
                start: 1,
                end: 6,
                resource_id: 4
            },
            Entry {
                start: 6,
                end: 12,
                resource_id: 3
            }
        ]),
        Report {
            conflict_pairs: 9,
            affected_bookings: 9,
            longest_overlap: 5
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            start: 1,
            end: 3,
            resource_id: 4
        }]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 0,
                end: 4,
                resource_id: 4
            },
            Entry {
                start: 2,
                end: 5,
                resource_id: 1
            }
        ]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 8,
                end: 10,
                resource_id: 4
            },
            Entry {
                start: 7,
                end: 10,
                resource_id: 1
            },
            Entry {
                start: 7,
                end: 9,
                resource_id: 2
            }
        ]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 4,
                end: 9,
                resource_id: 3
            },
            Entry {
                start: 8,
                end: 12,
                resource_id: 5
            },
            Entry {
                start: 0,
                end: 3,
                resource_id: 3
            },
            Entry {
                start: 8,
                end: 15,
                resource_id: 5
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 4
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 10,
                end: 12,
                resource_id: 1
            },
            Entry {
                start: 10,
                end: 11,
                resource_id: 4
            },
            Entry {
                start: 6,
                end: 9,
                resource_id: 3
            },
            Entry {
                start: 10,
                end: 14,
                resource_id: 1
            },
            Entry {
                start: 8,
                end: 9,
                resource_id: 1
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 2
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 5,
                end: 9,
                resource_id: 1
            },
            Entry {
                start: 3,
                end: 7,
                resource_id: 3
            },
            Entry {
                start: 4,
                end: 10,
                resource_id: 1
            },
            Entry {
                start: 9,
                end: 16,
                resource_id: 5
            },
            Entry {
                start: 10,
                end: 12,
                resource_id: 5
            },
            Entry {
                start: 9,
                end: 16,
                resource_id: 1
            }
        ]),
        Report {
            conflict_pairs: 3,
            affected_bookings: 5,
            longest_overlap: 4
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 3,
                end: 8,
                resource_id: 2
            },
            Entry {
                start: 3,
                end: 5,
                resource_id: 3
            },
            Entry {
                start: 0,
                end: 6,
                resource_id: 5
            },
            Entry {
                start: 2,
                end: 9,
                resource_id: 4
            },
            Entry {
                start: 8,
                end: 13,
                resource_id: 1
            },
            Entry {
                start: 8,
                end: 10,
                resource_id: 1
            },
            Entry {
                start: 10,
                end: 11,
                resource_id: 1
            }
        ]),
        Report {
            conflict_pairs: 2,
            affected_bookings: 3,
            longest_overlap: 2
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 7,
                end: 14,
                resource_id: 3
            },
            Entry {
                start: 8,
                end: 15,
                resource_id: 1
            },
            Entry {
                start: 2,
                end: 9,
                resource_id: 2
            },
            Entry {
                start: 10,
                end: 11,
                resource_id: 3
            },
            Entry {
                start: 3,
                end: 5,
                resource_id: 1
            },
            Entry {
                start: 5,
                end: 7,
                resource_id: 2
            },
            Entry {
                start: 10,
                end: 12,
                resource_id: 3
            },
            Entry {
                start: 4,
                end: 9,
                resource_id: 4
            }
        ]),
        Report {
            conflict_pairs: 4,
            affected_bookings: 5,
            longest_overlap: 2
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 0,
                end: 2,
                resource_id: 1
            },
            Entry {
                start: 2,
                end: 4,
                resource_id: 1
            }
        ]),
        Report {
            conflict_pairs: 0,
            affected_bookings: 0,
            longest_overlap: 0
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 0,
                end: 3,
                resource_id: 1
            },
            Entry {
                start: 0,
                end: 3,
                resource_id: 1
            },
            Entry {
                start: 0,
                end: 3,
                resource_id: 2
            }
        ]),
        Report {
            conflict_pairs: 1,
            affected_bookings: 2,
            longest_overlap: 3
        },
        "fixture 22"
    );
    assert_eq!(
        solve(&[
            Entry {
                start: 0,
                end: 4,
                resource_id: 1
            },
            Entry {
                start: 4,
                end: 9,
                resource_id: 1
            },
            Entry {
                start: 2,
                end: 6,
                resource_id: 1
            },
            Entry {
                start: 0,
                end: 8,
                resource_id: 2
            }
        ]),
        Report {
            conflict_pairs: 2,
            affected_bookings: 3,
            longest_overlap: 2
        },
        "fixture 23"
    );
}
