package benchmark

type Entry struct {
	Sensor    int64
	Operation int64
	Timestamp int64
}

type Report struct {
	Completed int64
	Active    int64
	Rejected  int64
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

func step(operation int64, timestamp int64, opened int64) int64 {
	if timestamp < 0 {
		return (-2)
	}
	if (operation == 0) && (opened == (-1)) {
		return timestamp
	}
	if (operation == 1) && (opened >= 0) && (timestamp >= opened) {
		return (-1)
	}
	return (-2)
}

func solve(rows []Entry) Report {
	var opened map[int64]int64 = make(map[int64]int64)
	var completed int64 = 0
	var active int64 = 0
	var rejected int64 = 0
	for _, row := range rows {
		var prior int64 = read(opened, row.Sensor, (-1))
		var next_time int64 = step(row.Operation, row.Timestamp, prior)
		if next_time == (-2) {
			rejected = (rejected + 1)
		} else {
			var updated_value_0 int64 = next_time
			opened = put(opened, row.Sensor, updated_value_0)
			if next_time == (-1) {
				completed = (completed + 1)
				active = (active - 1)
			} else {
				active = (active + 1)
			}
		}
	}
	return Report{Completed: completed, Active: active, Rejected: rejected}
}
