package benchmark

func SignedGCD(a int64, b int64) int64 {
	left, right := a, b
	if left < 0 {
		left = -left
	}
	if right < 0 {
		right = -right
	}
	for right != 0 {
		left, right = right, left%right
	}
	return left
}
