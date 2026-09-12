package benchmark

type Entry struct {
	Start  int64
	End    int64
	Demand int64
}

type Report struct {
	Peak             int64
	Earliest         int64
	OverloadedStarts int64
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

func load_at(rows []Entry, moment int64) int64 {
	var total int64 = 0
	for _, row := range rows {
		if (row.Start <= moment) && (row.End > moment) {
			total = (total + row.Demand)
		}
	}
	return total
}

func solve(rows []Entry, limit int64) Report {
	var seen map[int64]int64 = make(map[int64]int64)
	var peak int64 = 0
	var earliest int64 = (-1)
	var overloaded int64 = 0
	for _, row := range rows {
		var load int64 = load_at(rows, row.Start)
		if (load > peak) || ((load == peak) && (row.Start < earliest)) {
			peak = load
			earliest = row.Start
		}
		if read(seen, row.Start, 0) == 0 {
			if load > limit {
				overloaded = (overloaded + 1)
			}
			var updated_value_0 int64 = 1
			seen = put(seen, row.Start, updated_value_0)
		}
	}
	return Report{Peak: peak, Earliest: earliest, OverloadedStarts: overloaded}
}
