from dataclasses import dataclass


@dataclass(frozen=True)
class Active:
    pass


@dataclass(frozen=True)
class Suspended:
    reason: str


@dataclass(frozen=True)
class Locked:
    failed_attempts: int


@dataclass(frozen=True)
class Closed:
    pass


type AccountState = Active | Suspended | Locked | Closed


def can_sign_in(state: AccountState) -> bool:
    match state:
        case Active():
            return True
        case Suspended():
            return False
        case Locked():
            return False
        case Closed():
            return False


def status_label(state: AccountState) -> str:
    match state:
        case Active():
            return "active"
        case Suspended(reason):
            return f"suspended:{reason}"
        case Locked(failed_attempts):
            return f"locked:{failed_attempts}"
        case Closed():
            return "closed"


def suspend(state: AccountState, reason: str) -> AccountState:
    match state:
        case Active():
            return Suspended(reason)
        case Suspended(existing_reason):
            return Suspended(existing_reason)
        case Locked(failed_attempts):
            return Locked(failed_attempts)
        case Closed():
            return Closed()
