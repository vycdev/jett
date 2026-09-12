import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "accepted", value: ""}, 0);
check(solve("abc"), {kind: "accepted", value: "abc"}, 1);
check(solve("a+b"), {kind: "accepted", value: "a+b"}, 2);
check(solve("%41%20%7e"), {kind: "accepted", value: "A ~"}, 3);
check(solve("%25"), {kind: "accepted", value: "%"}, 4);
check(solve("%"), {kind: "rejected", error: "malformed"}, 5);
check(solve("%2"), {kind: "rejected", error: "malformed"}, 6);
check(solve("%GG"), {kind: "rejected", error: "malformed"}, 7);
check(solve("%00"), {kind: "rejected", error: "range"}, 8);
check(solve("%7F"), {kind: "rejected", error: "range"}, 9);
check(solve("%ff"), {kind: "rejected", error: "range"}, 10);
check(solve("%00%G0"), {kind: "rejected", error: "range"}, 11);
check(solve("%G0%00"), {kind: "rejected", error: "malformed"}, 12);
check(solve("x%2fy"), {kind: "accepted", value: "x/y"}, 13);
