import { solve, type TextOutcome } from "./solution.js";
function check(got: TextOutcome, want: TextOutcome, index: number): void {
  if (got.kind !== want.kind || (got.kind === "accepted" && want.kind === "accepted" && (got.value.length !== want.value.length || want.value.some((v, p) => v !== got.value[p]))) || (got.kind === "rejected" && want.kind === "rejected" && got.error !== want.error)) throw new Error(`case ${index}`);
}
check(solve(""), {kind: "accepted", value: [""]}, 0);
check(solve(","), {kind: "accepted", value: ["", ""]}, 1);
check(solve("a,b,c"), {kind: "accepted", value: ["a", "b", "c"]}, 2);
check(solve("\"a,b\",c"), {kind: "accepted", value: ["a,b", "c"]}, 3);
check(solve("\"\""), {kind: "accepted", value: [""]}, 4);
check(solve("\"a\"\"b\",,"), {kind: "accepted", value: ["a\"b", "", ""]}, 5);
check(solve("\""), {kind: "rejected", error: "malformed"}, 6);
check(solve("a\"b"), {kind: "rejected", error: "malformed"}, 7);
check(solve("\"a\" x"), {kind: "rejected", error: "malformed"}, 8);
check(solve("\"a\","), {kind: "accepted", value: ["a", ""]}, 9);
check(solve("\"\"\"\""), {kind: "accepted", value: ["\""]}, 10);
check(solve("\"\"\""), {kind: "rejected", error: "malformed"}, 11);
check(solve("a,b,,"), {kind: "accepted", value: ["a", "b", "", ""]}, 12);
