package benchmark

type MaybeInt struct {
	Found bool
	Value int64
}

func FirstDuplicate(values []int64) MaybeInt {
	seen := make(map[int64]struct{})
	for _, value := range values {
		if _, found := seen[value]; found {
			return MaybeInt{Found: true, Value: value}
		}
		seen[value] = struct{}{}
	}
	return MaybeInt{Found: false}
}
