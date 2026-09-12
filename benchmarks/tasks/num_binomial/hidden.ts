import { binomial } from "./solution.js";

if (binomial(0n, 0n) !== 1n) throw new Error("case 0");
if (binomial(0n, 1n) !== 0n) throw new Error("case 1");
if (binomial(1n, 0n) !== 1n) throw new Error("case 2");
if (binomial(1n, 1n) !== 1n) throw new Error("case 3");
if (binomial(1n, 2n) !== 0n) throw new Error("case 4");
if (binomial(5n, 2n) !== 10n) throw new Error("case 5");
if (binomial(5n, 3n) !== 10n) throw new Error("case 6");
if (binomial(10n, 1n) !== 10n) throw new Error("case 7");
if (binomial(10n, 9n) !== 10n) throw new Error("case 8");
if (binomial(20n, 10n) !== 184756n) throw new Error("case 9");
if (binomial(30n, 15n) !== 155117520n) throw new Error("case 10");
if (binomial(30n, 0n) !== 1n) throw new Error("case 11");
if (binomial(30n, 30n) !== 1n) throw new Error("case 12");
if (binomial(30n, 35n) !== 0n) throw new Error("case 13");
