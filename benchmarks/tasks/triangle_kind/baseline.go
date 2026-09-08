package benchmark

func TriangleKind(a int64, b int64, c int64) string {
	if a <= 0 || b <= 0 || c <= 0 {
		return "invalid"
	}
	if a+b <= c || a+c <= b || b+c <= a {
		return "invalid"
	}
	if a == b && b == c {
		return "equilateral"
	}
	if a == b || a == c || b == c {
		return "isosceles"
	}
	return "scalene"
}
