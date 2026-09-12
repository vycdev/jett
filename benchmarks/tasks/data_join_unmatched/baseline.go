package benchmark

type Entry struct {
	Key      int64
	Side     int64
	Quantity int64
}

type Report struct {
	Matched   int64
	LeftOnly  int64
	RightOnly int64
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
	var left map[int64]int64 = make(map[int64]int64)
	var right map[int64]int64 = make(map[int64]int64)
	for _, row := range rows {
		if row.Side == 0 {
			var updated_value_0 int64 = (read(left, row.Key, 0) + row.Quantity)
			left = put(left, row.Key, updated_value_0)
		}
		if row.Side == 1 {
			var updated_value_1 int64 = (read(right, row.Key, 0) + row.Quantity)
			right = put(right, row.Key, updated_value_1)
		}
	}
	var seen map[int64]int64 = make(map[int64]int64)
	var matched int64 = 0
	var left_only int64 = 0
	var right_only int64 = 0
	for _, row := range rows {
		if read(seen, row.Key, 0) == 0 {
			var updated_value_2 int64 = 1
			seen = put(seen, row.Key, updated_value_2)
			var a int64 = read(left, row.Key, 0)
			var b int64 = read(right, row.Key, 0)
			var pairs int64 = a
			if b < pairs {
				pairs = b
			}
			matched = (matched + pairs)
			left_only = ((left_only + a) - pairs)
			right_only = ((right_only + b) - pairs)
		}
	}
	return Report{Matched: matched, LeftOnly: left_only, RightOnly: right_only}
}
