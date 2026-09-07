package benchmark

type Interval struct {
	Start int64
	End   int64
}

func MergeSortedIntervals(intervals []Interval) []Interval {
	merged := make([]Interval, 0, len(intervals))
	for _, interval := range intervals {
		if len(merged) == 0 || interval.Start > merged[len(merged)-1].End {
			merged = append(merged, Interval{Start: interval.Start, End: interval.End})
		} else if interval.End > merged[len(merged)-1].End {
			merged[len(merged)-1].End = interval.End
		}
	}
	return merged
}
