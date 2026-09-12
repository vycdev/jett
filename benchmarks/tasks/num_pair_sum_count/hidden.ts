import { pairSumCount } from "./solution.js";

if (pairSumCount([], 0n) !== 0n) throw new Error("case 0");
if (pairSumCount([0n], 0n) !== 0n) throw new Error("case 1");
if (pairSumCount([0n, 0n], 0n) !== 1n) throw new Error("case 2");
if (pairSumCount([0n, 0n, 0n, 0n], 0n) !== 6n) throw new Error("case 3");
if (pairSumCount([1n, 2n, 3n, 4n], 5n) !== 2n) throw new Error("case 4");
if (pairSumCount([1n, 1n, 1n, 2n, 2n], 3n) !== 6n) throw new Error("case 5");
if (pairSumCount([-2n, -1n, 0n, 1n, 2n], 0n) !== 2n) throw new Error("case 6");
if (pairSumCount([5n, -5n, 5n, -5n], 0n) !== 4n) throw new Error("case 7");
if (pairSumCount([1n, 2n, 3n], 10n) !== 0n) throw new Error("case 8");
if (pairSumCount([1000n, 1000n], 2000n) !== 1n) throw new Error("case 9");
if (pairSumCount([1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n, 1n], 2n) !== 4950n) throw new Error("case 10");
if (pairSumCount([0n, 1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n, 10n, 11n, 12n, 13n, 14n, 15n, 16n, 17n, 18n, 19n], 19n) !== 10n) throw new Error("case 11");
if (pairSumCount([2n, 2n, 2n], 4n) !== 3n) throw new Error("case 12");
