package benchmark

import "strconv"

type AccountState interface{ isAccountState() }

type Active struct{}
type Suspended struct{ Reason string }
type Locked struct{ FailedAttempts int64 }
type Closed struct{}

func (Active) isAccountState()    {}
func (Suspended) isAccountState() {}
func (Locked) isAccountState()    {}
func (Closed) isAccountState()    {}

func CanSignIn(state AccountState) bool {
	switch state.(type) {
	case Active:
		return true
	case Suspended:
		return false
	case Locked:
		return false
	case Closed:
		return false
	}
	return false
}

func StatusLabel(state AccountState) string {
	switch state := state.(type) {
	case Active:
		return "active"
	case Suspended:
		return "suspended:" + state.Reason
	case Locked:
		return "locked:" + strconv.FormatInt(state.FailedAttempts, 10)
	case Closed:
		return "closed"
	}
	return "closed"
}

func Suspend(state AccountState, reason string) AccountState {
	switch state := state.(type) {
	case Active:
		return Suspended{Reason: reason}
	case Suspended:
		return state
	case Locked:
		return state
	case Closed:
		return state
	}
	return state
}
