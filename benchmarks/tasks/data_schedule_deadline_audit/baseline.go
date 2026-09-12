package benchmark

type Entry struct {
	Duration int64
	Deadline int64
	Penalty  int64
}

type Report struct {
	Late              int64
	WeightedTardiness int64
	Finish            int64
}

func solve(rows []Entry, limit int64) Report {
	var late int64 = 0
	var weighted int64 = 0
	var finish int64 = limit
	for _, row := range rows {
		finish = (finish + row.Duration)
		if finish > row.Deadline {
			late = (late + 1)
			weighted = (weighted + ((finish - row.Deadline) * row.Penalty))
		}
	}
	return Report{Late: late, WeightedTardiness: weighted, Finish: finish}
}
