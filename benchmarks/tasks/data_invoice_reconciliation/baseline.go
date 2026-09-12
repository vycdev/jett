package benchmark

type Entry struct {
	Invoice int64
	Amount  int64
}

type Report struct {
	Settled     int64
	Outstanding int64
	Credit      int64
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
	var sums map[int64]int64 = make(map[int64]int64)
	for _, row := range rows {
		var updated_value_0 int64 = (read(sums, row.Invoice, 0) + row.Amount)
		sums = put(sums, row.Invoice, updated_value_0)
	}
	var seen map[int64]int64 = make(map[int64]int64)
	var settled int64 = 0
	var outstanding int64 = 0
	var credit int64 = 0
	for _, row := range rows {
		if read(seen, row.Invoice, 0) == 0 {
			var updated_value_1 int64 = 1
			seen = put(seen, row.Invoice, updated_value_1)
			var balance int64 = read(sums, row.Invoice, 0)
			if balance == 0 {
				settled = (settled + 1)
			}
			if balance > 0 {
				outstanding = (outstanding + balance)
			}
			if balance < 0 {
				credit = (credit - balance)
			}
		}
	}
	return Report{Settled: settled, Outstanding: outstanding, Credit: credit}
}
