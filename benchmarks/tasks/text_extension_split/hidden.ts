import { solve, type TextPair } from "./solution.js";
function check(got: TextPair, want: TextPair, index: number): void {
  if (got.first !== want.first || got.second !== want.second) throw new Error(`case ${index}`);
}
check(solve(""), {first: "", second: ""}, 0);
check(solve("a"), {first: "a", second: ""}, 1);
check(solve("a.txt"), {first: "a", second: "txt"}, 2);
check(solve("a.tar.gz"), {first: "a.tar", second: "gz"}, 3);
check(solve(".env"), {first: ".env", second: ""}, 4);
check(solve(".env.local"), {first: ".env", second: "local"}, 5);
check(solve("a."), {first: "a", second: ""}, 6);
check(solve("."), {first: ".", second: ""}, 7);
check(solve(".."), {first: ".", second: ""}, 8);
check(solve("..."), {first: "..", second: ""}, 9);
check(solve("A.TXT"), {first: "A", second: "TXT"}, 10);
check(solve(" a .x"), {first: " a ", second: "x"}, 11);
