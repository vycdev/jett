package benchmark

type Entry struct {
	Prerequisite int64
	Dependent    int64
}

type Report struct {
	Layers         int64
	LastLayerCount int64
	LayerSum       int64
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
	var levels map[int64]int64 = make(map[int64]int64)
	var maximum int64 = (-1)
	var last_count int64 = 0
	var total int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		var level int64 = 0
		for _, row := range rows {
			if row.Dependent == vertex {
				var prior int64 = (read(levels, row.Prerequisite, 0) + 1)
				if prior > level {
					level = prior
				}
			}
		}
		var updated_value_0 int64 = level
		levels = put(levels, vertex, updated_value_0)
		total = (total + level)
		if level > maximum {
			maximum = level
			last_count = 0
		}
		if level == maximum {
			last_count = (last_count + 1)
		}
		vertex = (vertex + 1)
	}
	return Report{Layers: (maximum + 1), LastLayerCount: last_count, LayerSum: total}
}
