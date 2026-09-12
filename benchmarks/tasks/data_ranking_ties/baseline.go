package benchmark

type Entry struct {
	Score   int64
	Penalty int64
}

type Report struct {
	Selected  int64
	RankSum   int64
	TieGroups int64
}

func ahead(score int64, penalty int64, other_score int64, other_penalty int64) bool {
	return ((other_score > score) || ((other_score == score) && (other_penalty < penalty)))
}

func solve(rows []Entry, limit int64) Report {
	var selected int64 = 0
	var rank_sum int64 = 0
	var groups int64 = 0
	var index int64 = 0
	for _, row := range rows {
		var rank int64 = 1
		var ties int64 = 0
		var first int64 = index
		var other_index int64 = 0
		for _, other := range rows {
			if ahead(row.Score, row.Penalty, other.Score, other.Penalty) {
				rank = (rank + 1)
			}
			if (row.Score == other.Score) && (row.Penalty == other.Penalty) {
				ties = (ties + 1)
				if other_index < first {
					first = other_index
				}
			}
			other_index = (other_index + 1)
		}
		if rank <= limit {
			selected = (selected + 1)
			rank_sum = (rank_sum + rank)
		}
		if (ties > 1) && (first == index) {
			groups = (groups + 1)
		}
		index = (index + 1)
	}
	return Report{Selected: selected, RankSum: rank_sum, TieGroups: groups}
}
