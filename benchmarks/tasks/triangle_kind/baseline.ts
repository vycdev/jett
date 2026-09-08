export function triangleKind(a: number, b: number, c: number): string {
  if (a <= 0 || b <= 0 || c <= 0) return "invalid";
  if (a + b <= c || a + c <= b || b + c <= a) return "invalid";
  if (a === b && b === c) return "equilateral";
  if (a === b || a === c || b === c) return "isosceles";
  return "scalene";
}
