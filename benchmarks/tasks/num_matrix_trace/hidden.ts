import { matrixTrace } from "./solution.js";

if (matrixTrace([], 0n) !== 0n) throw new Error("case 0");
if (matrixTrace([0n], 1n) !== 0n) throw new Error("case 1");
if (matrixTrace([7n], 1n) !== 7n) throw new Error("case 2");
if (matrixTrace([-7n], 1n) !== -7n) throw new Error("case 3");
if (matrixTrace([1n, 2n, 3n, 4n], 2n) !== 5n) throw new Error("case 4");
if (matrixTrace([0n, 9n, 9n, 0n], 2n) !== 0n) throw new Error("case 5");
if (matrixTrace([-1n, 8n, 9n, -2n], 2n) !== -3n) throw new Error("case 6");
if (matrixTrace([1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n], 3n) !== 15n) throw new Error("case 7");
if (matrixTrace([0n, 0n, 0n, 0n, 1n, 0n, 0n, 0n, 0n], 3n) !== 1n) throw new Error("case 8");
if (matrixTrace([0n, 1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n, 10n, 11n, 12n, 13n, 14n, 15n], 4n) !== 30n) throw new Error("case 9");
if (matrixTrace([1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, -1000n, 1000n], 8n) !== 8000n) throw new Error("case 10");
if (matrixTrace([0n, -1n, 2n, -3n, 4n, -5n, 6n, -7n, 8n, -9n, 10n, -11n, 12n, -13n, 14n, -15n, 16n, -17n, 18n, -19n, 20n, -21n, 22n, -23n, 24n], 5n) !== 60n) throw new Error("case 11");
if (matrixTrace([0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n], 7n) !== 0n) throw new Error("case 12");
