import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "accepted", value: ""}, 0);
check(solve("41"), {kind: "accepted", value: "A"}, 1);
check(solve("6869"), {kind: "accepted", value: "hi"}, 2);
check(solve("207E"), {kind: "accepted", value: " ~"}, 3);
check(solve("5c22"), {kind: "accepted", value: "\\\""}, 4);
check(solve("0"), {kind: "rejected", error: "malformed"}, 5);
check(solve("GG"), {kind: "rejected", error: "malformed"}, 6);
check(solve("00"), {kind: "rejected", error: "range"}, 7);
check(solve("7f"), {kind: "rejected", error: "range"}, 8);
check(solve("ff"), {kind: "rejected", error: "range"}, 9);
check(solve("00g"), {kind: "rejected", error: "malformed"}, 10);
check(solve("002G"), {kind: "rejected", error: "range"}, 11);
check(solve("2G00"), {kind: "rejected", error: "malformed"}, 12);
