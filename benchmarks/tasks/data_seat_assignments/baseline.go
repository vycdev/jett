package benchmark

type Entry struct {
	Passenger int64
	Preferred int64
}

type Report struct {
	Assigned     int64
	Rejected     int64
	SeatChecksum int64
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

func choose_seat(occupied map[int64]int64, preferred int64, limit int64) int64 {
	if (preferred > 0) && (preferred <= limit) {
		if read(occupied, preferred, 0) == 0 {
			return preferred
		}
	}
	var seat int64 = 1
	for seat <= limit {
		if read(occupied, seat, 0) == 0 {
			return seat
		}
		seat = (seat + 1)
	}
	return 0
}

func solve(rows []Entry, limit int64) Report {
	var occupied map[int64]int64 = make(map[int64]int64)
	var passengers map[int64]int64 = make(map[int64]int64)
	var assigned int64 = 0
	var rejected int64 = 0
	var checksum int64 = 0
	for _, row := range rows {
		var seat int64 = 0
		if read(passengers, row.Passenger, 0) == 0 {
			seat = choose_seat(occupied, row.Preferred, limit)
		}
		if seat == 0 {
			rejected = (rejected + 1)
		} else {
			assigned = (assigned + 1)
			checksum = (checksum + ((row.Passenger + 1) * seat))
			var updated_value_0 int64 = 1
			occupied = put(occupied, seat, updated_value_0)
			var updated_value_1 int64 = 1
			passengers = put(passengers, row.Passenger, updated_value_1)
		}
	}
	return Report{Assigned: assigned, Rejected: rejected, SeatChecksum: checksum}
}
