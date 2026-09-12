package benchmark

type Entry struct {
	Source int64
	Target int64
}

type Report struct {
	Sources  int64
	Sinks    int64
	Balanced int64
}

func solve(rows []Entry, limit int64) Report {
	var sources int64 = 0
	var sinks int64 = 0
	var balanced int64 = 0
	var vertex int64 = 0
	for vertex < limit {
		var incoming int64 = 0
		var outgoing int64 = 0
		for _, row := range rows {
			if row.Source == vertex {
				outgoing = (outgoing + 1)
			}
			if row.Target == vertex {
				incoming = (incoming + 1)
			}
		}
		if (incoming == 0) && (outgoing > 0) {
			sources = (sources + 1)
		}
		if (outgoing == 0) && (incoming > 0) {
			sinks = (sinks + 1)
		}
		if incoming == outgoing {
			balanced = (balanced + 1)
		}
		vertex = (vertex + 1)
	}
	return Report{Sources: sources, Sinks: sinks, Balanced: balanced}
}
