import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "\"\"", 0);
check(solve("a"), "\"a\"", 1);
check(solve("\""), "\"\\\"\"", 2);
check(solve("\\"), "\"\\\\\"", 3);
check(solve("\n"), "\"\\n\"", 4);
check(solve("\t"), "\"\\t\"", 5);
check(solve("\r"), "\"\\r\"", 6);
check(solve("\b"), "\"\\b\"", 7);
check(solve("\f"), "\"\\f\"", 8);
check(solve("/"), "\"/\"", 9);
check(solve("a\"b\\c"), "\"a\\\"b\\\\c\"", 10);
check(solve("x\ny\tz"), "\"x\\ny\\tz\"", 11);
