import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "accepted", value: ""}, 0);
check(solve("abc"), {kind: "accepted", value: "abc"}, 1);
check(solve("\\n"), {kind: "accepted", value: "\n"}, 2);
check(solve("\\t\\r"), {kind: "accepted", value: "\t\r"}, 3);
check(solve("\\\\"), {kind: "accepted", value: "\\"}, 4);
check(solve("\\\""), {kind: "accepted", value: "\""}, 5);
check(solve("a\\nb"), {kind: "accepted", value: "a\nb"}, 6);
check(solve("\\"), {kind: "rejected", error: "malformed"}, 7);
check(solve("x\\q"), {kind: "rejected", error: "malformed"}, 8);
check(solve("\\0"), {kind: "rejected", error: "malformed"}, 9);
check(solve("\\\\n"), {kind: "accepted", value: "\\n"}, 10);
check(solve("a\nb"), {kind: "accepted", value: "a\nb"}, 11);
