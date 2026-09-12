package benchmark

type Entry struct {
	Stock       int64
	DailyDemand int64
}

type Report struct {
	RestockUnits   int64
	AtRisk         int64
	WorstShortfall int64
}

func solve(rows []Entry, limit int64) Report {
	var units int64 = 0
	var risk int64 = 0
	var worst int64 = 0
	for _, row := range rows {
		var shortage int64 = ((row.DailyDemand * limit) - row.Stock)
		if shortage > 0 {
			units = (units + shortage)
			risk = (risk + 1)
			if shortage > worst {
				worst = shortage
			}
		}
	}
	return Report{RestockUnits: units, AtRisk: risk, WorstShortfall: worst}
}
