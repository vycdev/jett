package benchmark

type Entry struct {
	Event int64
	Key   int64
	Delta int64
}

type Report struct {
	Applied    int64
	Duplicates int64
	Checksum   int64
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
	var seen map[int64]int64 = make(map[int64]int64)
	var applied int64 = 0
	var duplicates int64 = 0
	var checksum int64 = 0
	for _, row := range rows {
		if read(seen, row.Event, 0) == 0 {
			var updated_value_0 int64 = 1
			seen = put(seen, row.Event, updated_value_0)
			checksum = (checksum + ((row.Key + 1) * row.Delta))
			applied = (applied + 1)
		} else {
			duplicates = (duplicates + 1)
		}
	}
	return Report{Applied: applied, Duplicates: duplicates, Checksum: checksum}
}
