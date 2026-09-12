package benchmark

type Entry struct {
	ResourceId int64
	Timestamp  int64
	Duration   int64
}

type Report struct {
	Accepted   int64
	Rejected   int64
	ExpiresSum int64
}

func put(table map[int64]int64, key int64, value int64) map[int64]int64 {
	table[key] = value
	return table
}

func read(table map[int64]int64, key int64, fallback int64) int64 {
	value, exists := table[key]
	if exists {
		return value
	}
	return fallback
}

func solve(rows []Entry) Report {
	var expiries map[int64]int64 = make(map[int64]int64)
	var accepted int64 = 0
	var rejected int64 = 0
	var total int64 = 0
	for _, row := range rows {
		var prior int64 = read(expiries, row.ResourceId, 0)
		if (row.Timestamp >= prior) && (row.Duration > 0) {
			var next_expiry int64 = (row.Timestamp + row.Duration)
			total = ((total + next_expiry) - prior)
			var updated_value_0 int64 = next_expiry
			expiries = put(expiries, row.ResourceId, updated_value_0)
			accepted = (accepted + 1)
		} else {
			rejected = (rejected + 1)
		}
	}
	return Report{Accepted: accepted, Rejected: rejected, ExpiresSum: total}
}
