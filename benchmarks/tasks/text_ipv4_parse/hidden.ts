import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve("0.0.0.0"), {kind: "accepted", value: 0}, 0);
check(solve("255.255.255.255"), {kind: "accepted", value: 4294967295}, 1);
check(solve("127.0.0.1"), {kind: "accepted", value: 2130706433}, 2);
check(solve("192.168.1.1"), {kind: "accepted", value: 3232235777}, 3);
check(solve("1.2.3.4"), {kind: "accepted", value: 16909060}, 4);
check(solve(""), {kind: "rejected", error: "malformed"}, 5);
check(solve("1.2.3"), {kind: "rejected", error: "malformed"}, 6);
check(solve("1.2.3.4.5"), {kind: "rejected", error: "malformed"}, 7);
check(solve("01.2.3.4"), {kind: "rejected", error: "malformed"}, 8);
check(solve("256.0.0.1"), {kind: "rejected", error: "malformed"}, 9);
check(solve("1..2.3"), {kind: "rejected", error: "malformed"}, 10);
check(solve("1.2.3.-1"), {kind: "rejected", error: "malformed"}, 11);
check(solve("1.2.3.4 "), {kind: "rejected", error: "malformed"}, 12);
