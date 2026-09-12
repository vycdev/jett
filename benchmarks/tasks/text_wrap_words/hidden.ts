import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve("", 5), "", 0);
check(solve("   ", 3), "", 1);
check(solve("a b c", 3), "a b\nc", 2);
check(solve("a b c", 1), "a\nb\nc", 3);
check(solve("longword x", 3), "longword\nx", 4);
check(solve("one two three", 7), "one two\nthree", 5);
check(solve(" one  two ", 80), "one two", 6);
check(solve("ab cd ef", 5), "ab cd\nef", 7);
check(solve("abc d", 3), "abc\nd", 8);
check(solve("a bc", 4), "a bc", 9);
check(solve("a bc", 3), "a\nbc", 10);
check(solve("x longword y", 3), "x\nlongword\ny", 11);
