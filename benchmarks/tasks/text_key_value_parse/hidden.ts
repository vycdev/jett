import { solve, type TextPair } from "./solution.js";
function check(got: TextPair, want: TextPair, index: number): void {
  if (got.first !== want.first || got.second !== want.second) throw new Error(`case ${index}`);
}
check(solve("a=b"), {first: "a", second: "b"}, 0);
check(solve(" a = b "), {first: "a", second: "b"}, 1);
check(solve("a="), {first: "a", second: ""}, 2);
check(solve("a=  "), {first: "a", second: ""}, 3);
check(solve("a=b=c"), {first: "a", second: "b=c"}, 4);
check(solve("a=="), {first: "a", second: "="}, 5);
check(solve("x y=z z"), {first: "x y", second: "z z"}, 6);
check(solve("\ta\t=\tb\t"), {first: "\ta\t", second: "\tb\t"}, 7);
check(solve("x= a  b "), {first: "x", second: "a  b"}, 8);
check(solve("KEY=value"), {first: "KEY", second: "value"}, 9);
check(solve("a= = "), {first: "a", second: "="}, 10);
check(solve("  k  = v=x "), {first: "k", second: "v=x"}, 11);
