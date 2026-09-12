import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve(" "), " ", 1);
check(solve("HELLO"), "Hello", 2);
check(solve("hello WORLD"), "Hello World", 3);
check(solve("  a  B "), "  A  B ", 4);
check(solve("a-b"), "A-b", 5);
check(solve("1ABC"), "1abc", 6);
check(solve("!HELLO"), "!hello", 7);
check(solve("x\tY"), "X\ty", 8);
check(solve("a b c"), "A B C", 9);
check(solve("MiXeD"), "Mixed", 10);
check(solve("A  B  C"), "A  B  C", 11);
