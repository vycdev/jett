package benchmark

type Entry struct {
	Start int64
	End   int64
}

type Report struct {
	Covered    int64
	Gaps       int64
	LongestGap int64
}

func covered_at(rows []Entry, moment int64) bool {
	for _, row := range rows {
		if (row.Start <= moment) && (row.End > moment) {
			return true
		}
	}
	return false
}

func solve(rows []Entry, limit int64) Report {
	var covered int64 = 0
	var gaps int64 = 0
	var longest int64 = 0
	var current int64 = 0
	var moment int64 = 0
	for moment < limit {
		if covered_at(rows, moment) {
			covered = (covered + 1)
			current = 0
		} else {
			if current == 0 {
				gaps = (gaps + 1)
			}
			current = (current + 1)
			if current > longest {
				longest = current
			}
		}
		moment = (moment + 1)
	}
	return Report{Covered: covered, Gaps: gaps, LongestGap: longest}
}
