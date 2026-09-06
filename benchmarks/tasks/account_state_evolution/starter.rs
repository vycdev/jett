#[derive(Debug, PartialEq, Eq)]
pub enum AccountState {
    Active,
    Locked(i64),
    Closed,
}

pub fn can_sign_in(state: AccountState) -> bool {
    match state {
        AccountState::Active => true,
        AccountState::Locked(_) => false,
        AccountState::Closed => false,
    }
}

pub fn status_label(state: AccountState) -> String {
    match state {
        AccountState::Active => "active".to_string(),
        AccountState::Locked(failed_attempts) => format!("locked:{failed_attempts}"),
        AccountState::Closed => "closed".to_string(),
    }
}
