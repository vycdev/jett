package benchmark

import "strconv"

type AccountState interface{ isAccountState() }

type Active struct{}
type Locked struct{ FailedAttempts int64 }
type Closed struct{}

func (Active) isAccountState() {}
func (Locked) isAccountState() {}
func (Closed) isAccountState() {}

func CanSignIn(state AccountState) bool {
	switch state.(type) {
	case Active:
		return true
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
	case Locked:
		return "locked:" + strconv.FormatInt(state.FailedAttempts, 10)
	case Closed:
		return "closed"
	}
	return "closed"
}
