import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve("abc"), "abc", 1);
check(solve("#x"), "", 2);
check(solve("a#b"), "a", 3);
check(solve("a#b\nc"), "a\nc", 4);
check(solve("\"#x\"#y"), "\"#x\"", 5);
check(solve("#a\n#b\n"), "\n\n", 6);
check(solve("a # x"), "a ", 7);
check(solve("\"a\\\"#b\"#c"), "\"a\\\"#b\"", 8);
check(solve("\\#x"), "\\", 9);
check(solve("\"#open"), "\"#open", 10);
check(solve("a\n#x\nb"), "a\n\nb", 11);
