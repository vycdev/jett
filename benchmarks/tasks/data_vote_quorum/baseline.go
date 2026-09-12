package benchmark

type Entry struct {
	Voter  int64
	Choice int64
	Weight int64
}

type Report struct {
	YesWeight int64
	NoWeight  int64
	QuorumMet int64
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

func contribution(choice int64, weight int64, wanted int64) int64 {
	if (choice == wanted) && (weight > 0) {
		return weight
	}
	return 0
}

func solve(rows []Entry, limit int64) Report {
	var yes_votes map[int64]int64 = make(map[int64]int64)
	var no_votes map[int64]int64 = make(map[int64]int64)
	var yes int64 = 0
	var no int64 = 0
	for _, row := range rows {
		var next_yes int64 = contribution(row.Choice, row.Weight, 1)
		var next_no int64 = contribution(row.Choice, row.Weight, 0)
		yes = ((yes + next_yes) - read(yes_votes, row.Voter, 0))
		no = ((no + next_no) - read(no_votes, row.Voter, 0))
		var updated_value_0 int64 = next_yes
		yes_votes = put(yes_votes, row.Voter, updated_value_0)
		var updated_value_1 int64 = next_no
		no_votes = put(no_votes, row.Voter, updated_value_1)
	}
	var met int64 = 0
	if (yes >= limit) && (yes > no) {
		met = 1
	}
	return Report{YesWeight: yes, NoWeight: no, QuorumMet: met}
}
