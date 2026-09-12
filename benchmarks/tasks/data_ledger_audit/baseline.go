package benchmark

type Entry struct {
	Account int64
	Delta   int64
}

type Report struct {
	Total         int64
	LowestBalance int64
	FirstOverdraw int64
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

func solve(rows []Entry, limit int64) Report {
	var balances map[int64]int64 = make(map[int64]int64)
	var seen map[int64]int64 = make(map[int64]int64)
	var total int64 = 0
	var lowest int64 = limit
	var first int64 = (-1)
	var index int64 = 0
	for _, row := range rows {
		if read(seen, row.Account, 0) == 0 {
			total = (total + limit)
			var updated_value_0 int64 = 1
			seen = put(seen, row.Account, updated_value_0)
		}
		var balance int64 = (read(balances, row.Account, limit) + row.Delta)
		var updated_value_1 int64 = balance
		balances = put(balances, row.Account, updated_value_1)
		total = (total + row.Delta)
		if (index == 0) || (balance < lowest) {
			lowest = balance
		}
		if (balance < 0) && (first == (-1)) {
			first = index
		}
		index = (index + 1)
	}
	return Report{Total: total, LowestBalance: lowest, FirstOverdraw: first}
}
