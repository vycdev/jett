#[derive(Debug, PartialEq, Eq)]
pub enum AccountState {
    Active,
    Suspended(String),
    Locked(i64),
    Closed,
}

pub fn can_sign_in(state: AccountState) -> bool {
    match state {
        AccountState::Active => true,
        AccountState::Suspended(_) => false,
        AccountState::Locked(_) => false,
        AccountState::Closed => false,
    }
}

pub fn status_label(state: AccountState) -> String {
    match state {
        AccountState::Active => "active".to_string(),
        AccountState::Suspended(reason) => format!("suspended:{reason}"),
        AccountState::Locked(failed_attempts) => format!("locked:{failed_attempts}"),
        AccountState::Closed => "closed".to_string(),
    }
}

pub fn suspend(state: AccountState, reason: String) -> AccountState {
    match state {
        AccountState::Active => AccountState::Suspended(reason),
        AccountState::Suspended(existing_reason) => AccountState::Suspended(existing_reason),
        AccountState::Locked(failed_attempts) => AccountState::Locked(failed_attempts),
        AccountState::Closed => AccountState::Closed,
    }
}
