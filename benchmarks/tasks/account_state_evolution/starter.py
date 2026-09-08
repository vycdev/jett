from dataclasses import dataclass


@dataclass(frozen=True)
class Active:
    pass


@dataclass(frozen=True)
class Locked:
    failed_attempts: int


@dataclass(frozen=True)
class Closed:
    pass


type AccountState = Active | Locked | Closed


def can_sign_in(state: AccountState) -> bool:
    match state:
        case Active():
            return True
        case Locked():
            return False
        case Closed():
            return False


def status_label(state: AccountState) -> str:
    match state:
        case Active():
            return "active"
        case Locked(failed_attempts):
            return f"locked:{failed_attempts}"
        case Closed():
            return "closed"
