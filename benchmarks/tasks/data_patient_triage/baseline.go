package benchmark

type Entry struct {
	Patient  int64
	Severity int64
	Arrival  int64
}

type Report struct {
	Selected        int64
	PatientChecksum int64
	SeveritySum     int64
}

func precedes(severity int64, arrival int64, index int64, other_severity int64, other_arrival int64, other_index int64) bool {
	if other_severity != severity {
		return (other_severity > severity)
	}
	if other_arrival != arrival {
		return (other_arrival < arrival)
	}
	return (other_index < index)
}

func rank_of(rows []Entry, severity int64, arrival int64, index int64) int64 {
	var rank int64 = 1
	var other_index int64 = 0
	for _, other := range rows {
		if precedes(severity, arrival, index, other.Severity, other.Arrival, other_index) {
			rank = (rank + 1)
		}
		other_index = (other_index + 1)
	}
	return rank
}

func solve(rows []Entry, limit int64) Report {
	var selected int64 = 0
	var checksum int64 = 0
	var total int64 = 0
	var index int64 = 0
	for _, row := range rows {
		var rank int64 = rank_of(rows, row.Severity, row.Arrival, index)
		if rank <= limit {
			selected = (selected + 1)
			checksum = (checksum + (rank * (row.Patient + 1)))
			total = (total + row.Severity)
		}
		index = (index + 1)
	}
	return Report{Selected: selected, PatientChecksum: checksum, SeveritySum: total}
}
