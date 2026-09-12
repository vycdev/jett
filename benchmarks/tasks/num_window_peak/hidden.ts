import { windowPeak } from "./solution.js";

if (windowPeak([], 0n) !== null) throw new Error("case 0");
if (windowPeak([], 1n) !== null) throw new Error("case 1");
if (windowPeak([7n], 0n) !== null) throw new Error("case 2");
if (windowPeak([7n], 1n) !== 7n) throw new Error("case 3");
if (windowPeak([7n], 2n) !== null) throw new Error("case 4");
if (windowPeak([1n, 2n, 3n, 4n], 2n) !== 7n) throw new Error("case 5");
if (windowPeak([-5n, -2n, -7n], 2n) !== -7n) throw new Error("case 6");
if (windowPeak([2n, -1n, 2n, -1n, 2n], 3n) !== 3n) throw new Error("case 7");
if (windowPeak([5n, -9n, 5n], 1n) !== 5n) throw new Error("case 8");
if (windowPeak([5n, -9n, 5n], 3n) !== 1n) throw new Error("case 9");
if (windowPeak([0n, 0n, 0n], 2n) !== 0n) throw new Error("case 10");
if (windowPeak([10n, -5n, -5n, 10n], 2n) !== 5n) throw new Error("case 11");
if (windowPeak([0n, 1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n, 10n, 11n, 12n, 13n, 14n, 15n, 16n, 17n, 18n, 19n], 5n) !== 85n) throw new Error("case 12");
if (windowPeak([20n, 19n, 18n, 17n, 16n, 15n, 14n, 13n, 12n, 11n, 10n, 9n, 8n, 7n, 6n, 5n, 4n, 3n, 2n, 1n, 0n], 7n) !== 119n) throw new Error("case 13");
