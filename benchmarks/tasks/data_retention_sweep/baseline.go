package benchmark

type Entry struct {
	Key      int64
	Modified int64
	Size     int64
}

type Report struct {
	Removed   int64
	Reclaimed int64
	Retained  int64
}

func newest_index(rows []Entry, key int64) int64 {
	var best_index int64 = (-1)
	var best_time int64 = (-1000000)
	var index int64 = 0
	for _, row := range rows {
		if (row.Key == key) && (row.Modified >= best_time) {
			best_index = index
			best_time = row.Modified
		}
		index = (index + 1)
	}
	return best_index
}

func solve(rows []Entry, limit int64) Report {
	var removed int64 = 0
	var reclaimed int64 = 0
	var retained int64 = 0
	var index int64 = 0
	for _, row := range rows {
		if (row.Modified < limit) && (index != newest_index(rows, row.Key)) {
			removed = (removed + 1)
			reclaimed = (reclaimed + row.Size)
		} else {
			retained = (retained + 1)
		}
		index = (index + 1)
	}
	return Report{Removed: removed, Reclaimed: reclaimed, Retained: retained}
}
