import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "rejected", error: "empty"}, 0);
check(solve("0"), {kind: "accepted", value: "0"}, 1);
check(solve("-000"), {kind: "accepted", value: "0"}, 2);
check(solve("+0012"), {kind: "accepted", value: "12"}, 3);
check(solve("-012"), {kind: "accepted", value: "-12"}, 4);
check(solve("123"), {kind: "accepted", value: "123"}, 5);
check(solve("+"), {kind: "rejected", error: "malformed"}, 6);
check(solve("--1"), {kind: "rejected", error: "malformed"}, 7);
check(solve(" 1"), {kind: "rejected", error: "malformed"}, 8);
check(solve("1.0"), {kind: "rejected", error: "malformed"}, 9);
check(solve("0x0"), {kind: "rejected", error: "malformed"}, 10);
check(solve("9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999"), {kind: "accepted", value: "9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999"}, 11);
check(solve("-0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"), {kind: "accepted", value: "-1"}, 12);
