from dataclasses import dataclass
from enum import Enum, auto


class OrderState(Enum):
    DRAFT = auto()
    SUBMITTED = auto()
    PAID = auto()
    SHIPPED = auto()
    CANCELLED = auto()


class OrderEvent(Enum):
    SUBMIT = auto()
    PAY = auto()
    SHIP = auto()
    CANCEL = auto()


class TransitionError(Enum):
    EVENT_NOT_ALLOWED = auto()
    TERMINAL_STATE = auto()


@dataclass(frozen=True)
class Accepted:
    state: OrderState


@dataclass(frozen=True)
class Rejected:
    error: TransitionError


type TransitionResult = Accepted | Rejected


def transition_from_draft(event: OrderEvent) -> TransitionResult:
    match event:
        case OrderEvent.SUBMIT:
            return Accepted(OrderState.SUBMITTED)
        case OrderEvent.PAY:
            return Rejected(TransitionError.EVENT_NOT_ALLOWED)
        case OrderEvent.SHIP:
            return Rejected(TransitionError.EVENT_NOT_ALLOWED)
        case OrderEvent.CANCEL:
            return Accepted(OrderState.CANCELLED)


def transition_from_submitted(event: OrderEvent) -> TransitionResult:
    match event:
        case OrderEvent.SUBMIT:
            return Rejected(TransitionError.EVENT_NOT_ALLOWED)
        case OrderEvent.PAY:
            return Accepted(OrderState.PAID)
        case OrderEvent.SHIP:
            return Rejected(TransitionError.EVENT_NOT_ALLOWED)
        case OrderEvent.CANCEL:
            return Accepted(OrderState.CANCELLED)


def transition_from_paid(event: OrderEvent) -> TransitionResult:
    match event:
        case OrderEvent.SUBMIT:
            return Rejected(TransitionError.EVENT_NOT_ALLOWED)
        case OrderEvent.PAY:
            return Rejected(TransitionError.EVENT_NOT_ALLOWED)
        case OrderEvent.SHIP:
            return Accepted(OrderState.SHIPPED)
        case OrderEvent.CANCEL:
            return Accepted(OrderState.CANCELLED)


def reject_terminal(event: OrderEvent) -> TransitionResult:
    match event:
        case OrderEvent.SUBMIT:
            return Rejected(TransitionError.TERMINAL_STATE)
        case OrderEvent.PAY:
            return Rejected(TransitionError.TERMINAL_STATE)
        case OrderEvent.SHIP:
            return Rejected(TransitionError.TERMINAL_STATE)
        case OrderEvent.CANCEL:
            return Rejected(TransitionError.TERMINAL_STATE)


def transition(state: OrderState, event: OrderEvent) -> TransitionResult:
    match state:
        case OrderState.DRAFT:
            return transition_from_draft(event)
        case OrderState.SUBMITTED:
            return transition_from_submitted(event)
        case OrderState.PAID:
            return transition_from_paid(event)
        case OrderState.SHIPPED:
            return reject_terminal(event)
        case OrderState.CANCELLED:
            return reject_terminal(event)
