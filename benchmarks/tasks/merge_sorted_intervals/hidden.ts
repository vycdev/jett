import { mergeSortedIntervals, type Interval } from "./solution.js";

function equal(actual: readonly Interval[], expected: readonly Interval[]): boolean {
  return actual.length === expected.length && actual.every((value, index) => {
    const wanted = expected[index];
    return wanted !== undefined && value.start === wanted.start && value.end === wanted.end;
  });
}

if (!equal(mergeSortedIntervals([]), [])) throw new Error("empty");
if (!equal(mergeSortedIntervals([{ start: 1n, end: 3n }]), [{ start: 1n, end: 3n }])) throw new Error("single");
if (!equal(mergeSortedIntervals([{ start: 1n, end: 3n }, { start: 2n, end: 6n }, { start: 8n, end: 10n }]), [{ start: 1n, end: 6n }, { start: 8n, end: 10n }])) throw new Error("overlap");
if (!equal(mergeSortedIntervals([{ start: 1n, end: 10n }, { start: 2n, end: 3n }, { start: 10n, end: 12n }]), [{ start: 1n, end: 12n }])) throw new Error("nested and touching");
if (!equal(mergeSortedIntervals([{ start: -5n, end: -2n }, { start: -2n, end: 0n }, { start: 4n, end: 4n }]), [{ start: -5n, end: 0n }, { start: 4n, end: 4n }])) throw new Error("negative");
