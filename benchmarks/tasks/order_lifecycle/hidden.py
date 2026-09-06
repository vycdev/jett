from solution import (
    Accepted,
    OrderEvent,
    OrderState,
    Rejected,
    TransitionError,
    TransitionResult,
    transition,
)


def check(actual: TransitionResult, expected: TransitionResult) -> None:
    assert actual == expected


check(transition(OrderState.DRAFT, OrderEvent.SUBMIT), Accepted(OrderState.SUBMITTED))
check(transition(OrderState.DRAFT, OrderEvent.PAY), Rejected(TransitionError.EVENT_NOT_ALLOWED))
check(transition(OrderState.DRAFT, OrderEvent.SHIP), Rejected(TransitionError.EVENT_NOT_ALLOWED))
check(transition(OrderState.DRAFT, OrderEvent.CANCEL), Accepted(OrderState.CANCELLED))
check(transition(OrderState.SUBMITTED, OrderEvent.SUBMIT), Rejected(TransitionError.EVENT_NOT_ALLOWED))
check(transition(OrderState.SUBMITTED, OrderEvent.PAY), Accepted(OrderState.PAID))
check(transition(OrderState.SUBMITTED, OrderEvent.SHIP), Rejected(TransitionError.EVENT_NOT_ALLOWED))
check(transition(OrderState.SUBMITTED, OrderEvent.CANCEL), Accepted(OrderState.CANCELLED))
check(transition(OrderState.PAID, OrderEvent.SUBMIT), Rejected(TransitionError.EVENT_NOT_ALLOWED))
check(transition(OrderState.PAID, OrderEvent.PAY), Rejected(TransitionError.EVENT_NOT_ALLOWED))
check(transition(OrderState.PAID, OrderEvent.SHIP), Accepted(OrderState.SHIPPED))
check(transition(OrderState.PAID, OrderEvent.CANCEL), Accepted(OrderState.CANCELLED))
check(transition(OrderState.SHIPPED, OrderEvent.SUBMIT), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.SHIPPED, OrderEvent.PAY), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.SHIPPED, OrderEvent.SHIP), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.SHIPPED, OrderEvent.CANCEL), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.CANCELLED, OrderEvent.SUBMIT), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.CANCELLED, OrderEvent.PAY), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.CANCELLED, OrderEvent.SHIP), Rejected(TransitionError.TERMINAL_STATE))
check(transition(OrderState.CANCELLED, OrderEvent.CANCEL), Rejected(TransitionError.TERMINAL_STATE))
