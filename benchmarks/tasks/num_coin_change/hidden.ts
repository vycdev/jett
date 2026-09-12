import { coinChange } from "./solution.js";

if (coinChange([], 0n) !== 0n) throw new Error("case 0");
if (coinChange([], 1n) !== -1n) throw new Error("case 1");
if (coinChange([1n], 0n) !== 0n) throw new Error("case 2");
if (coinChange([1n], 7n) !== 7n) throw new Error("case 3");
if (coinChange([2n], 3n) !== -1n) throw new Error("case 4");
if (coinChange([2n], 8n) !== 4n) throw new Error("case 5");
if (coinChange([1n, 3n, 4n], 6n) !== 2n) throw new Error("case 6");
if (coinChange([5n, 2n], 11n) !== 4n) throw new Error("case 7");
if (coinChange([3n, 7n], 10n) !== 2n) throw new Error("case 8");
if (coinChange([3n, 7n], 5n) !== -1n) throw new Error("case 9");
if (coinChange([2n, 2n, 4n], 8n) !== 2n) throw new Error("case 10");
if (coinChange([50n], 100n) !== 2n) throw new Error("case 11");
if (coinChange([7n, 10n, 25n], 99n) !== 6n) throw new Error("case 12");
if (coinChange([9n, 6n, 5n, 1n], 11n) !== 2n) throw new Error("case 13");
