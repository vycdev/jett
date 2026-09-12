package benchmark

type Entry struct {
	Start int64
	End   int64
}

type Report struct {
	Rooms              int64
	AssignmentChecksum int64
	Reuses             int64
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
	var ends map[int64]int64 = make(map[int64]int64)
	var rooms int64 = 0
	var checksum int64 = 0
	var reuses int64 = 0
	var index int64 = 1
	for _, row := range rows {
		var selected int64 = 0
		var room int64 = 1
		for room <= rooms {
			if (selected == 0) && (read(ends, room, 0) <= row.Start) {
				selected = room
			}
			room = (room + 1)
		}
		if selected == 0 {
			rooms = (rooms + 1)
			selected = rooms
		} else {
			reuses = (reuses + 1)
		}
		var updated_value_0 int64 = row.End
		ends = put(ends, selected, updated_value_0)
		checksum = (checksum + (index * selected))
		index = (index + 1)
	}
	return Report{Rooms: rooms, AssignmentChecksum: checksum, Reuses: reuses}
}
