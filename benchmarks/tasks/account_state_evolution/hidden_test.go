package benchmark

import (
	"reflect"
	"testing"
)

func TestAccountStateEvolution(t *testing.T) {
	if !CanSignIn(Active{}) || CanSignIn(Suspended{Reason: "review"}) || CanSignIn(Locked{FailedAttempts: 3}) || CanSignIn(Closed{}) {
		t.Fatal("unexpected sign-in policy")
	}
	labels := []struct {
		state AccountState
		want  string
	}{
		{Active{}, "active"},
		{Suspended{Reason: "review"}, "suspended:review"},
		{Locked{FailedAttempts: 3}, "locked:3"},
		{Closed{}, "closed"},
	}
	for _, label := range labels {
		if got := StatusLabel(label.state); got != label.want {
			t.Fatalf("got %q, want %q", got, label.want)
		}
	}
	states := []struct {
		state  AccountState
		reason string
		want   AccountState
	}{
		{Active{}, "maintenance", Suspended{Reason: "maintenance"}},
		{Suspended{Reason: "review"}, "replacement", Suspended{Reason: "review"}},
		{Locked{FailedAttempts: 7}, "ignored", Locked{FailedAttempts: 7}},
		{Closed{}, "ignored", Closed{}},
	}
	for _, state := range states {
		if got := Suspend(state.state, state.reason); !reflect.DeepEqual(got, state.want) {
			t.Fatalf("got %#v, want %#v", got, state.want)
		}
	}
}
