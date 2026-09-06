package benchmark

func BoundedWeightedSum(values []int64, cap int64) int64 {
	var total int64
	for index, value := range values {
		bounded := value
		if bounded > cap {
			bounded = cap
		} else if bounded < -cap {
			bounded = -cap
		}
		total += bounded * int64(index+1)
	}
	return total
}
