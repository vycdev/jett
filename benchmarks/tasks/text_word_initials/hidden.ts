import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve("  "), "", 1);
check(solve("alice"), "A", 2);
check(solve("alice bob"), "AB", 3);
check(solve("  alice   bob "), "AB", 4);
check(solve("a b c"), "ABC", 5);
check(solve("1 one"), "1O", 6);
check(solve("! bang"), "!B", 7);
check(solve("mixed CASE"), "MC", 8);
check(solve("a\tb c"), "AC", 9);
check(solve("z Z"), "ZZ", 10);
check(solve("foo-bar baz"), "FB", 11);
