package benchmark

type Entry struct {
	Job          int64
	Prerequisite int64
	Cost         int64
}

type Report struct {
	Ready     int64
	Blocked   int64
	TotalCost int64
}

func ready_job(rows []Entry, job int64) bool {
	var current int64 = job
	for current != 0 {
		var parent int64 = (-1)
		for _, row := range rows {
			if row.Job == current {
				parent = row.Prerequisite
			}
		}
		if parent == (-1) {
			return false
		}
		current = parent
	}
	return true
}

func solve(rows []Entry) Report {
	var ready int64 = 0
	var blocked int64 = 0
	var total int64 = 0
	for _, row := range rows {
		if ready_job(rows, row.Job) {
			ready = (ready + 1)
			total = (total + row.Cost)
		} else {
			blocked = (blocked + 1)
		}
	}
	return Report{Ready: ready, Blocked: blocked, TotalCost: total}
}
