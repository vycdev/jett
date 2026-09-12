package benchmark

type Entry struct {
	Node   int64
	Parent int64
}

type Report struct {
	Ancestor         int64
	SelectedDistance int64
	LargestDistance  int64
}

func parent_of(rows []Entry, node int64) int64 {
	for _, row := range rows {
		if row.Node == node {
			return row.Parent
		}
	}
	return (-1)
}

func solve(rows []Entry, limit int64) Report {
	var largest int64 = 0
	for _, row := range rows {
		if row.Node > largest {
			largest = row.Node
		}
	}
	if parent_of(rows, limit) == (-1) {
		return Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
	}
	var selected int64 = limit
	var selected_distance int64 = 0
	for selected > 0 {
		var other int64 = largest
		var other_distance int64 = 0
		for other > 0 {
			if other == selected {
				return Report{Ancestor: selected, SelectedDistance: selected_distance, LargestDistance: other_distance}
			}
			other = parent_of(rows, other)
			other_distance = (other_distance + 1)
		}
		selected = parent_of(rows, selected)
		selected_distance = (selected_distance + 1)
	}
	return Report{Ancestor: 0, SelectedDistance: (-1), LargestDistance: (-1)}
}
