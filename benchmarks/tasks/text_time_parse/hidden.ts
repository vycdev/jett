import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve("00:00:00"), {kind: "accepted", value: 0}, 0);
check(solve("23:59:59"), {kind: "accepted", value: 86399}, 1);
check(solve("12:34:56"), {kind: "accepted", value: 45296}, 2);
check(solve("01:02:03"), {kind: "accepted", value: 3723}, 3);
check(solve("24:00:00"), {kind: "rejected", error: "range"}, 4);
check(solve("00:60:00"), {kind: "rejected", error: "range"}, 5);
check(solve("00:00:60"), {kind: "rejected", error: "range"}, 6);
check(solve("0:00:00"), {kind: "rejected", error: "malformed"}, 7);
check(solve("00-00-00"), {kind: "rejected", error: "malformed"}, 8);
check(solve("aa:00:00"), {kind: "rejected", error: "malformed"}, 9);
check(solve("99:xx:00"), {kind: "rejected", error: "malformed"}, 10);
check(solve(""), {kind: "rejected", error: "malformed"}, 11);
