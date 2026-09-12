import { integerSqrt } from "./solution.js";

if (integerSqrt(0n) !== 0n) throw new Error("case 0");
if (integerSqrt(1n) !== 1n) throw new Error("case 1");
if (integerSqrt(2n) !== 1n) throw new Error("case 2");
if (integerSqrt(3n) !== 1n) throw new Error("case 3");
if (integerSqrt(4n) !== 2n) throw new Error("case 4");
if (integerSqrt(8n) !== 2n) throw new Error("case 5");
if (integerSqrt(9n) !== 3n) throw new Error("case 6");
if (integerSqrt(10n) !== 3n) throw new Error("case 7");
if (integerSqrt(15n) !== 3n) throw new Error("case 8");
if (integerSqrt(16n) !== 4n) throw new Error("case 9");
if (integerSqrt(17n) !== 4n) throw new Error("case 10");
if (integerSqrt(99980000n) !== 9998n) throw new Error("case 11");
if (integerSqrt(99980001n) !== 9999n) throw new Error("case 12");
if (integerSqrt(99980002n) !== 9999n) throw new Error("case 13");
if (integerSqrt(999999999999n) !== 999999n) throw new Error("case 14");
if (integerSqrt(1000000000000n) !== 1000000n) throw new Error("case 15");
