export function firstDuplicate(values: readonly bigint[]): bigint | null {
  const seen = new Set<bigint>();
  for (const value of values) {
    if (seen.has(value)) return value;
    seen.add(value);
  }
  return null;
}
