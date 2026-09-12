package benchmark

type Entry struct {
	Source int64
	Target int64
	Cost   int64
}

type Report struct {
	Reachable     int64
	CostSum       int64
	MostExpensive int64
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
	var costs map[int64]int64 = make(map[int64]int64)
	var updated_value_0 int64 = 0
	costs = put(costs, 0, updated_value_0)
	var reachable int64 = 0
	var total int64 = 0
	var maximum int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		var best int64 = read(costs, vertex, 1000000)
		for _, row := range rows {
			if row.Target == vertex {
				var candidate int64 = (read(costs, row.Source, 1000000) + row.Cost)
				if candidate < best {
					best = candidate
				}
			}
		}
		var updated_value_1 int64 = best
		costs = put(costs, vertex, updated_value_1)
		if best < 1000000 {
			reachable = (reachable + 1)
			total = (total + best)
			if best > maximum {
				maximum = best
			}
		}
		vertex = (vertex + 1)
	}
	return Report{Reachable: reachable, CostSum: total, MostExpensive: maximum}
}
