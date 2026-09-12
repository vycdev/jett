import { polynomialValue } from "./solution.js";

if (polynomialValue([], 3n) !== 0n) throw new Error("case 0");
if (polynomialValue([5n], 0n) !== 5n) throw new Error("case 1");
if (polynomialValue([5n], -10n) !== 5n) throw new Error("case 2");
if (polynomialValue([1n, 2n, 3n], 0n) !== 1n) throw new Error("case 3");
if (polynomialValue([1n, 2n, 3n], 1n) !== 6n) throw new Error("case 4");
if (polynomialValue([1n, 2n, 3n], 2n) !== 17n) throw new Error("case 5");
if (polynomialValue([1n, 2n, 3n], -2n) !== 9n) throw new Error("case 6");
if (polynomialValue([0n, 0n, 1n], 3n) !== 9n) throw new Error("case 7");
if (polynomialValue([1n, -1n, 1n, -1n], -1n) !== 4n) throw new Error("case 8");
if (polynomialValue([100n, 100n, 100n, 100n, 100n, 100n, 100n, 100n, 100n, 100n, 100n, 100n], 10n) !== 11111111111100n) throw new Error("case 9");
if (polynomialValue([-100n, -100n, -100n, -100n, -100n, -100n, -100n, -100n, -100n, -100n, -100n, -100n], -10n) !== 9090909090900n) throw new Error("case 10");
if (polynomialValue([0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n], 10n) !== 0n) throw new Error("case 11");
if (polynomialValue([3n, 0n, -4n, 0n, 5n], 2n) !== 67n) throw new Error("case 12");
