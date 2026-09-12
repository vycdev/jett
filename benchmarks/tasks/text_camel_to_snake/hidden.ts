import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve("a"), "a", 1);
check(solve("A"), "a", 2);
check(solve("camelCase"), "camel_case", 3);
check(solve("HTTPServer"), "http_server", 4);
check(solve("XML"), "xml", 5);
check(solve("parseURLValue"), "parse_url_value", 6);
check(solve("x2Y"), "x2_y", 7);
check(solve("ABc"), "a_bc", 8);
check(solve("oneTwoThree"), "one_two_three", 9);
check(solve("v123"), "v123", 10);
check(solve("ABCdEF"), "ab_cd_ef", 11);
