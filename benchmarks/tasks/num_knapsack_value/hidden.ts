import { knapsackValue } from "./solution.js";

if (knapsackValue([], [], 0n) !== 0n) throw new Error("case 0");
if (knapsackValue([], [], 8n) !== 0n) throw new Error("case 1");
if (knapsackValue([1n], [7n], 0n) !== 0n) throw new Error("case 2");
if (knapsackValue([5n], [10n], 4n) !== 0n) throw new Error("case 3");
if (knapsackValue([5n], [10n], 5n) !== 10n) throw new Error("case 4");
if (knapsackValue([2n, 3n, 4n], [4n, 5n, 7n], 5n) !== 9n) throw new Error("case 5");
if (knapsackValue([2n, 2n, 2n], [3n, 3n, 3n], 4n) !== 6n) throw new Error("case 6");
if (knapsackValue([1n, 1n, 1n], [0n, 5n, 7n], 2n) !== 12n) throw new Error("case 7");
if (knapsackValue([6n, 3n, 4n, 2n], [30n, 14n, 16n, 9n], 10n) !== 46n) throw new Error("case 8");
if (knapsackValue([10n, 20n, 30n], [60n, 100n, 100n], 40n) !== 160n) throw new Error("case 9");
if (knapsackValue([1n, 3n, 4n], [1n, 4n, 5n], 7n) !== 9n) throw new Error("case 10");
if (knapsackValue([40n], [100n], 40n) !== 100n) throw new Error("case 11");
if (knapsackValue([7n, 6n, 5n, 4n, 3n, 2n], [5n, 6n, 7n, 8n, 9n, 10n], 12n) !== 27n) throw new Error("case 12");
