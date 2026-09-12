package benchmark

type Entry struct {
	Start      int64
	End        int64
	ResourceId int64
}

type Report struct {
	ConflictPairs    int64
	AffectedBookings int64
	LongestOverlap   int64
}

func overlap(a_start int64, a_end int64, b_start int64, b_end int64) int64 {
	var start int64 = a_start
	var end int64 = a_end
	if b_start > start {
		start = b_start
	}
	if b_end < end {
		end = b_end
	}
	if end > start {
		return (end - start)
	}
	return 0
}

func solve(rows []Entry) Report {
	var pairs int64 = 0
	var affected int64 = 0
	var longest int64 = 0
	var left_index int64 = 0
	for _, left := range rows {
		var hit bool = false
		var right_index int64 = 0
		for _, right := range rows {
			var duration int64 = 0
			if (left_index != right_index) && (left.ResourceId == right.ResourceId) {
				duration = overlap(left.Start, left.End, right.Start, right.End)
			}
			if duration > 0 {
				hit = true
				if left_index < right_index {
					pairs = (pairs + 1)
				}
				if duration > longest {
					longest = duration
				}
			}
			right_index = (right_index + 1)
		}
		if hit {
			affected = (affected + 1)
		}
		left_index = (left_index + 1)
	}
	return Report{ConflictPairs: pairs, AffectedBookings: affected, LongestOverlap: longest}
}
