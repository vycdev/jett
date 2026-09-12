package benchmark

type Entry struct {
	Source int64
	Target int64
}

type Report struct {
	DistanceSum int64
	Farthest    int64
	Unreachable int64
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

func relax(rows []Entry, distance map[int64]int64) map[int64]int64 {
	var updated map[int64]int64 = make(map[int64]int64)
	for _, row := range rows {
		var prior int64 = read(distance, row.Source, 1000)
		var old int64 = read(distance, row.Target, 1000)
		var best int64 = read(updated, row.Target, old)
		if (prior + 1) < best {
			var updated_value_0 int64 = (prior + 1)
			updated = put(updated, row.Target, updated_value_0)
		}
	}
	return updated
}

func solve(rows []Entry, limit int64) Report {
	var distance map[int64]int64 = make(map[int64]int64)
	var updated_value_1 int64 = 0
	distance = put(distance, 0, updated_value_1)
	var turn int64 = 0
	for turn < limit {
		var updated map[int64]int64 = relax(rows, distance)
		var vertex int64 = 0
		for vertex < limit {
			var updated_value_2 int64 = read(updated, vertex, read(distance, vertex, 1000))
			distance = put(distance, vertex, updated_value_2)
			vertex = (vertex + 1)
		}
		turn = (turn + 1)
	}
	var total int64 = 0
	var farthest int64 = 0
	var missing int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		var hops int64 = read(distance, vertex, 1000)
		if hops == 1000 {
			missing = (missing + 1)
		} else {
			total = (total + hops)
			if hops > farthest {
				farthest = hops
			}
		}
		vertex = (vertex + 1)
	}
	return Report{DistanceSum: total, Farthest: farthest, Unreachable: missing}
}
