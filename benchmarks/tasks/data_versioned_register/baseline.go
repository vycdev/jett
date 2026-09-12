package benchmark

type Entry struct {
	Key     int64
	Version int64
	Value   int64
}

type Report struct {
	Accepted int64
	Stale    int64
	Checksum int64
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
	var versions map[int64]int64 = make(map[int64]int64)
	var values map[int64]int64 = make(map[int64]int64)
	var accepted int64 = 0
	var stale int64 = 0
	var checksum int64 = 0
	for _, row := range rows {
		if row.Version > read(versions, row.Key, (-1)) {
			checksum = (checksum + ((row.Key + 1) * (row.Value - read(values, row.Key, 0))))
			var updated_value_0 int64 = row.Value
			values = put(values, row.Key, updated_value_0)
			var updated_value_1 int64 = row.Version
			versions = put(versions, row.Key, updated_value_1)
			accepted = (accepted + 1)
		} else {
			stale = (stale + 1)
		}
	}
	return Report{Accepted: accepted, Stale: stale, Checksum: checksum}
}
