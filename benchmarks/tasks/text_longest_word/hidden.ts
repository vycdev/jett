import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && got.value !== want.value) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "rejected", error: "empty"}, 0);
check(solve("  "), {kind: "rejected", error: "empty"}, 1);
check(solve("a"), {kind: "accepted", value: "a"}, 2);
check(solve("a bb c"), {kind: "accepted", value: "bb"}, 3);
check(solve("ab cd"), {kind: "accepted", value: "ab"}, 4);
check(solve(" xxx yy "), {kind: "accepted", value: "xxx"}, 5);
check(solve("a\tb zz"), {kind: "accepted", value: "a\tb"}, 6);
check(solve("! ??"), {kind: "accepted", value: "??"}, 7);
check(solve("long short"), {kind: "accepted", value: "short"}, 8);
check(solve("same same"), {kind: "accepted", value: "same"}, 9);
check(solve("   end"), {kind: "accepted", value: "end"}, 10);
check(solve("ab abc abcd"), {kind: "accepted", value: "abcd"}, 11);
