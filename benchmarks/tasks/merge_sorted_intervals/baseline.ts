export type Interval = { readonly start: bigint; readonly end: bigint };

export function mergeSortedIntervals(intervals: readonly Interval[]): readonly Interval[] {
  const merged: Interval[] = [];
  for (const interval of intervals) {
    const current = merged[merged.length - 1];
    if (current === undefined || interval.start > current.end) {
      merged.push({ start: interval.start, end: interval.end });
    } else if (interval.end > current.end) {
      merged[merged.length - 1] = { start: current.start, end: interval.end };
    }
  }
  return merged;
}
