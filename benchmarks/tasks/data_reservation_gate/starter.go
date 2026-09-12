package benchmark

type Entry struct {
	Party int64
	Seats int64
}

type Report struct {
	Accepted int64
	Rejected int64
	Occupied int64
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

func solve(rows []Entry, limit int64) Report {
	var parties map[int64]int64 = make(map[int64]int64)
	var accepted int64 = 0
	var rejected int64 = 0
	var occupied int64 = 0
	for _, row := range rows {
		var invalid bool = ((row.Seats <= 0) || (read(parties, row.Party, 0) == 1))
		if invalid || ((occupied + row.Seats) >= limit) {
			rejected = (rejected + 1)
		} else {
			accepted = (accepted + 1)
			occupied = (occupied + row.Seats)
			var updated_value_0 int64 = 1
			parties = put(parties, row.Party, updated_value_0)
		}
	}
	return Report{Accepted: accepted, Rejected: rejected, Occupied: occupied}
}
