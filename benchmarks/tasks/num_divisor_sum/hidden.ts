import { divisorSum } from "./solution.js";

if (divisorSum(1n) !== 1n) throw new Error("case 0");
if (divisorSum(2n) !== 3n) throw new Error("case 1");
if (divisorSum(3n) !== 4n) throw new Error("case 2");
if (divisorSum(4n) !== 7n) throw new Error("case 3");
if (divisorSum(6n) !== 12n) throw new Error("case 4");
if (divisorSum(12n) !== 28n) throw new Error("case 5");
if (divisorSum(16n) !== 31n) throw new Error("case 6");
if (divisorSum(25n) !== 31n) throw new Error("case 7");
if (divisorSum(36n) !== 91n) throw new Error("case 8");
if (divisorSum(49n) !== 57n) throw new Error("case 9");
if (divisorSum(64n) !== 127n) throw new Error("case 10");
if (divisorSum(97n) !== 98n) throw new Error("case 11");
if (divisorSum(120n) !== 360n) throw new Error("case 12");
if (divisorSum(360n) !== 1170n) throw new Error("case 13");
if (divisorSum(99991n) !== 99992n) throw new Error("case 14");
if (divisorSum(100000n) !== 246078n) throw new Error("case 15");
