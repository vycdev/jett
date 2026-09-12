package benchmark

type Entry struct {
	Node   int64
	Parent int64
	Weight int64
}

type Report struct {
	Roots         int64
	MaxDepth      int64
	WeightedDepth int64
}

func depth_of(rows []Entry, node int64) int64 {
	var current int64 = node
	var depth int64 = (-1)
	for current != 0 {
		var parent int64 = 0
		for _, row := range rows {
			if row.Node == current {
				parent = row.Parent
			}
		}
		current = parent
		depth = (depth + 1)
	}
	return depth
}

func solve(rows []Entry) Report {
	var roots int64 = 0
	var maximum int64 = 0
	var total int64 = 0
	for _, row := range rows {
		var depth int64 = depth_of(rows, row.Node)
		if depth == 0 {
			roots = (roots + 1)
		}
		if depth > maximum {
			maximum = depth
		}
		total = (total + (row.Weight * depth))
	}
	return Report{Roots: roots, MaxDepth: maximum, WeightedDepth: total}
}
