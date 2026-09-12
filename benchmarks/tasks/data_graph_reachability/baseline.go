package benchmark

type Entry struct {
	Source int64
	Target int64
}

type Report struct {
	Reachable          int64
	Unreachable        int64
	ReachableEdgeCount int64
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
	var seen map[int64]int64 = make(map[int64]int64)
	if limit > 0 {
		var updated_value_0 int64 = 1
		seen = put(seen, 0, updated_value_0)
	}
	var turn int64 = 0
	for turn < limit {
		for _, row := range rows {
			if read(seen, row.Source, 0) == 1 {
				var updated_value_1 int64 = 1
				seen = put(seen, row.Target, updated_value_1)
			}
		}
		turn = (turn + 1)
	}
	var reachable int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		reachable = (reachable + read(seen, vertex, 0))
		vertex = (vertex + 1)
	}
	var edges int64 = 0
	for _, row := range rows {
		edges = (edges + read(seen, row.Source, 0))
	}
	return Report{Reachable: reachable, Unreachable: (limit - reachable), ReachableEdgeCount: edges}
}
