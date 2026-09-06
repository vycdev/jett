include!("solution.rs");

#[test]
fn hidden_account_state_evolution() {
    assert!(can_sign_in(AccountState::Active));
    assert!(!can_sign_in(AccountState::Suspended("review".to_string())));
    assert!(!can_sign_in(AccountState::Locked(3)));
    assert!(!can_sign_in(AccountState::Closed));
    assert_eq!(status_label(AccountState::Active), "active");
    assert_eq!(
        status_label(AccountState::Suspended("review".to_string())),
        "suspended:review"
    );
    assert_eq!(status_label(AccountState::Locked(3)), "locked:3");
    assert_eq!(status_label(AccountState::Closed), "closed");
    assert_eq!(
        suspend(AccountState::Active, "maintenance".to_string()),
        AccountState::Suspended("maintenance".to_string())
    );
    assert_eq!(
        suspend(
            AccountState::Suspended("review".to_string()),
            "replacement".to_string()
        ),
        AccountState::Suspended("review".to_string())
    );
    assert_eq!(
        suspend(AccountState::Locked(7), "ignored".to_string()),
        AccountState::Locked(7)
    );
    assert_eq!(
        suspend(AccountState::Closed, "ignored".to_string()),
        AccountState::Closed
    );
}
