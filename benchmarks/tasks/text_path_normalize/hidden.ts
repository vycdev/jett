import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve("/"), "/", 0);
check(solve("///"), "/", 1);
check(solve("/a/b"), "/a/b", 2);
check(solve("/a//b/"), "/a/b", 3);
check(solve("/./a/."), "/a", 4);
check(solve("/a/../b"), "/b", 5);
check(solve("/../../a"), "/a", 6);
check(solve("/a/b/../../"), "/", 7);
check(solve("/.../.."), "/", 8);
check(solve("/a/..."), "/a/...", 9);
check(solve("/a/ /b"), "/a/ /b", 10);
check(solve("/a/../a/./b/.."), "/a", 11);
