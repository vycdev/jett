import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "accepted", value: ""}, 0);
check(solve("1A"), {kind: "accepted", value: "A"}, 1);
check(solve("3A2B"), {kind: "accepted", value: "AAABB"}, 2);
check(solve("1A1A"), {kind: "accepted", value: "AA"}, 3);
check(solve("100Z"), {kind: "accepted", value: "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"}, 4);
check(solve("0A"), {kind: "rejected", error: "malformed"}, 5);
check(solve("01A"), {kind: "rejected", error: "malformed"}, 6);
check(solve("101A"), {kind: "rejected", error: "malformed"}, 7);
check(solve("2a"), {kind: "rejected", error: "malformed"}, 8);
check(solve("A"), {kind: "rejected", error: "malformed"}, 9);
check(solve("12"), {kind: "rejected", error: "malformed"}, 10);
check(solve("60A41B"), {kind: "rejected", error: "range"}, 11);
check(solve("2A0B"), {kind: "rejected", error: "malformed"}, 12);
