import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "rejected", error: "empty"}, 0);
check(solve("0"), {kind: "accepted", value: 0}, 1);
check(solve("1"), {kind: "accepted", value: 1}, 2);
check(solve("00101"), {kind: "accepted", value: 5}, 3);
check(solve("1111111111111111"), {kind: "accepted", value: 65535}, 4);
check(solve("1000000000000000"), {kind: "accepted", value: 32768}, 5);
check(solve("00000000000000000"), {kind: "rejected", error: "range"}, 6);
check(solve("2"), {kind: "rejected", error: "malformed"}, 7);
check(solve("10x"), {kind: "rejected", error: "malformed"}, 8);
check(solve(" 1"), {kind: "rejected", error: "malformed"}, 9);
check(solve("+1"), {kind: "rejected", error: "malformed"}, 10);
check(solve("101010"), {kind: "accepted", value: 42}, 11);
check(solve("xxxxxxxxxxxxxxxxx"), {kind: "rejected", error: "range"}, 12);
