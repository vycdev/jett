package benchmark

type Entry struct {
	Node   int64
	Parent int64
	Weight int64
}

type Report struct {
	NodeCount   int64
	TotalWeight int64
	LeafCount   int64
}

func belongs(rows []Entry, node int64, selected int64) bool {
	var current int64 = node
	for current != 0 {
		if current == selected {
			return true
		}
		var parent int64 = 0
		for _, row := range rows {
			if row.Node == current {
				parent = row.Parent
			}
		}
		current = parent
	}
	return false
}

func is_leaf(rows []Entry, node int64) bool {
	for _, row := range rows {
		if row.Parent == node {
			return false
		}
	}
	return true
}

func solve(rows []Entry, limit int64) Report {
	var count int64 = 0
	var total int64 = 0
	var leaves int64 = 0
	for _, row := range rows {
		if belongs(rows, row.Node, limit) {
			count = (count + 1)
			total = (total + row.Weight)
			if is_leaf(rows, row.Node) {
				leaves = (leaves + 1)
			}
		}
	}
	return Report{NodeCount: count, TotalWeight: total, LeafCount: leaves}
}
