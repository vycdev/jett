package benchmark

type Entry struct {
	Source int64
	Target int64
}

type Report struct {
	Components int64
	Largest    int64
	Isolated   int64
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

func spread(rows []Entry, seen map[int64]int64, vertex int64) int64 {
	var found int64 = read(seen, vertex, 0)
	for _, row := range rows {
		if row.Source == vertex {
			if read(seen, row.Target, 0) == 1 {
				found = 1
			}
		}
		if row.Target == vertex {
			if read(seen, row.Source, 0) == 1 {
				found = 1
			}
		}
	}
	return found
}

func component_size(rows []Entry, start int64, limit int64) int64 {
	var seen map[int64]int64 = make(map[int64]int64)
	var updated_value_0 int64 = 1
	seen = put(seen, start, updated_value_0)
	var turn int64 = 0
	for turn < limit {
		var vertex int64 = 0
		for vertex < limit {
			var updated_value_1 int64 = spread(rows, seen, vertex)
			seen = put(seen, vertex, updated_value_1)
			vertex = (vertex + 1)
		}
		turn = (turn + 1)
	}
	var count int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		if read(seen, vertex, 0) == 1 {
			if vertex < start {
				return 0
			}
			count = (count + 1)
		}
		vertex = (vertex + 1)
	}
	return count
}

func solve(rows []Entry, limit int64) Report {
	var components int64 = 0
	var largest int64 = 0
	var isolated int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		var count int64 = component_size(rows, vertex, limit)
		if count > 0 {
			components = (components + 1)
		}
		if count > largest {
			largest = count
		}
		var incident int64 = 0
		for _, row := range rows {
			if (row.Source == vertex) || (row.Target == vertex) {
				incident = (incident + 1)
			}
		}
		if incident == 0 {
			isolated = (isolated + 1)
		}
		vertex = (vertex + 1)
	}
	return Report{Components: components, Largest: largest, Isolated: isolated}
}
