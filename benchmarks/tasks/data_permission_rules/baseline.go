package benchmark

type Entry struct {
	Principal   int64
	Allowed     int64
	Specificity int64
}

type Report struct {
	Allowed   int64
	Denied    int64
	Defaulted int64
}

func solve(rows []Entry, limit int64) Report {
	var allowed int64 = 0
	var denied int64 = 0
	var defaulted int64 = 0
	var principal int64 = 0
	for principal < limit {
		var priority int64 = (-1)
		var decision int64 = 0
		for _, row := range rows {
			if (row.Principal == principal) && (row.Specificity >= priority) {
				priority = row.Specificity
				decision = row.Allowed
			}
		}
		if priority == (-1) {
			defaulted = (defaulted + 1)
		}
		allowed = (allowed + decision)
		denied = ((denied + 1) - decision)
		principal = (principal + 1)
	}
	return Report{Allowed: allowed, Denied: denied, Defaulted: defaulted}
}
