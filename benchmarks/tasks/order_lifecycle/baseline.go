package benchmark

type OrderState uint8

const (
	Draft OrderState = iota
	Submitted
	Paid
	Shipped
	Cancelled
)

type OrderEvent uint8

const (
	Submit OrderEvent = iota
	Pay
	Ship
	Cancel
)

type TransitionError uint8

const (
	EventNotAllowed TransitionError = iota
	TerminalState
)

type TransitionResult interface {
	isTransitionResult()
}

type Accepted struct {
	State OrderState
}

func (Accepted) isTransitionResult() {}

type Rejected struct {
	Error TransitionError
}

func (Rejected) isTransitionResult() {}

func transitionFromDraft(event OrderEvent) TransitionResult {
	switch event {
	case Submit:
		return Accepted{State: Submitted}
	case Pay:
		return Rejected{Error: EventNotAllowed}
	case Ship:
		return Rejected{Error: EventNotAllowed}
	case Cancel:
		return Accepted{State: Cancelled}
	}
	return Rejected{Error: EventNotAllowed}
}

func transitionFromSubmitted(event OrderEvent) TransitionResult {
	switch event {
	case Submit:
		return Rejected{Error: EventNotAllowed}
	case Pay:
		return Accepted{State: Paid}
	case Ship:
		return Rejected{Error: EventNotAllowed}
	case Cancel:
		return Accepted{State: Cancelled}
	}
	return Rejected{Error: EventNotAllowed}
}

func transitionFromPaid(event OrderEvent) TransitionResult {
	switch event {
	case Submit:
		return Rejected{Error: EventNotAllowed}
	case Pay:
		return Rejected{Error: EventNotAllowed}
	case Ship:
		return Accepted{State: Shipped}
	case Cancel:
		return Accepted{State: Cancelled}
	}
	return Rejected{Error: EventNotAllowed}
}

func rejectTerminal(event OrderEvent) TransitionResult {
	switch event {
	case Submit:
		return Rejected{Error: TerminalState}
	case Pay:
		return Rejected{Error: TerminalState}
	case Ship:
		return Rejected{Error: TerminalState}
	case Cancel:
		return Rejected{Error: TerminalState}
	}
	return Rejected{Error: TerminalState}
}

func Transition(state OrderState, event OrderEvent) TransitionResult {
	switch state {
	case Draft:
		return transitionFromDraft(event)
	case Submitted:
		return transitionFromSubmitted(event)
	case Paid:
		return transitionFromPaid(event)
	case Shipped:
		return rejectTerminal(event)
	case Cancelled:
		return rejectTerminal(event)
	}
	return Rejected{Error: TerminalState}
}
