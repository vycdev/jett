from solution import Active, Closed, Locked, Suspended, can_sign_in, status_label, suspend


assert can_sign_in(Active())
assert not can_sign_in(Suspended("review"))
assert not can_sign_in(Locked(3))
assert not can_sign_in(Closed())
assert status_label(Active()) == "active"
assert status_label(Suspended("review")) == "suspended:review"
assert status_label(Locked(3)) == "locked:3"
assert status_label(Closed()) == "closed"
assert suspend(Active(), "maintenance") == Suspended("maintenance")
assert suspend(Suspended("review"), "replacement") == Suspended("review")
assert suspend(Locked(7), "ignored") == Locked(7)
assert suspend(Closed(), "ignored") == Closed()
