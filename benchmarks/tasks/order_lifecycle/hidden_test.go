package benchmark

import (
	"reflect"
	"testing"
)

func TestOrderLifecycle(t *testing.T) {
	tests := []struct {
		state OrderState
		event OrderEvent
		want  TransitionResult
	}{
		{Draft, Submit, Accepted{State: Submitted}},
		{Draft, Pay, Rejected{Error: EventNotAllowed}},
		{Draft, Ship, Rejected{Error: EventNotAllowed}},
		{Draft, Cancel, Accepted{State: Cancelled}},
		{Submitted, Submit, Rejected{Error: EventNotAllowed}},
		{Submitted, Pay, Accepted{State: Paid}},
		{Submitted, Ship, Rejected{Error: EventNotAllowed}},
		{Submitted, Cancel, Accepted{State: Cancelled}},
		{Paid, Submit, Rejected{Error: EventNotAllowed}},
		{Paid, Pay, Rejected{Error: EventNotAllowed}},
		{Paid, Ship, Accepted{State: Shipped}},
		{Paid, Cancel, Accepted{State: Cancelled}},
		{Shipped, Submit, Rejected{Error: TerminalState}},
		{Shipped, Pay, Rejected{Error: TerminalState}},
		{Shipped, Ship, Rejected{Error: TerminalState}},
		{Shipped, Cancel, Rejected{Error: TerminalState}},
		{Cancelled, Submit, Rejected{Error: TerminalState}},
		{Cancelled, Pay, Rejected{Error: TerminalState}},
		{Cancelled, Ship, Rejected{Error: TerminalState}},
		{Cancelled, Cancel, Rejected{Error: TerminalState}},
	}
	for _, test := range tests {
		if got := Transition(test.state, test.event); !reflect.DeepEqual(got, test.want) {
			t.Fatalf("state %v event %v: got %#v, want %#v", test.state, test.event, got, test.want)
		}
	}
}
