#[derive(Debug, PartialEq, Eq)]
pub enum OrderState {
    Draft,
    Submitted,
    Paid,
    Shipped,
    Cancelled,
}

pub enum OrderEvent {
    Submit,
    Pay,
    Ship,
    Cancel,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransitionError {
    EventNotAllowed,
    TerminalState,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransitionResult {
    Accepted(OrderState),
    Rejected(TransitionError),
}

fn transition_from_draft(event: OrderEvent) -> TransitionResult {
    match event {
        OrderEvent::Submit => TransitionResult::Accepted(OrderState::Submitted),
        OrderEvent::Pay => TransitionResult::Rejected(TransitionError::EventNotAllowed),
        OrderEvent::Ship => TransitionResult::Rejected(TransitionError::EventNotAllowed),
        OrderEvent::Cancel => TransitionResult::Accepted(OrderState::Cancelled),
    }
}

fn transition_from_submitted(event: OrderEvent) -> TransitionResult {
    match event {
        OrderEvent::Submit => TransitionResult::Rejected(TransitionError::EventNotAllowed),
        OrderEvent::Pay => TransitionResult::Accepted(OrderState::Paid),
        OrderEvent::Ship => TransitionResult::Rejected(TransitionError::EventNotAllowed),
        OrderEvent::Cancel => TransitionResult::Accepted(OrderState::Cancelled),
    }
}

fn transition_from_paid(event: OrderEvent) -> TransitionResult {
    match event {
        OrderEvent::Submit => TransitionResult::Rejected(TransitionError::EventNotAllowed),
        OrderEvent::Pay => TransitionResult::Rejected(TransitionError::EventNotAllowed),
        OrderEvent::Ship => TransitionResult::Accepted(OrderState::Shipped),
        OrderEvent::Cancel => TransitionResult::Accepted(OrderState::Cancelled),
    }
}

fn reject_terminal(event: OrderEvent) -> TransitionResult {
    match event {
        OrderEvent::Submit => TransitionResult::Rejected(TransitionError::TerminalState),
        OrderEvent::Pay => TransitionResult::Rejected(TransitionError::TerminalState),
        OrderEvent::Ship => TransitionResult::Rejected(TransitionError::TerminalState),
        OrderEvent::Cancel => TransitionResult::Rejected(TransitionError::TerminalState),
    }
}

pub fn transition(state: OrderState, event: OrderEvent) -> TransitionResult {
    match state {
        OrderState::Draft => transition_from_draft(event),
        OrderState::Submitted => transition_from_submitted(event),
        OrderState::Paid => transition_from_paid(event),
        OrderState::Shipped => reject_terminal(event),
        OrderState::Cancelled => reject_terminal(event),
    }
}
