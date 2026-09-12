import { modularPower } from "./solution.js";

if (modularPower(0n, 0n, 7n) !== 1n) throw new Error("case 0");
if (modularPower(0n, 1n, 7n) !== 0n) throw new Error("case 1");
if (modularPower(7n, 0n, 1n) !== 0n) throw new Error("case 2");
if (modularPower(2n, 10n, 1000n) !== 24n) throw new Error("case 3");
if (modularPower(3n, 4n, 5n) !== 1n) throw new Error("case 4");
if (modularPower(999999n, 60n, 1000000n) !== 1n) throw new Error("case 5");
if (modularPower(1000000n, 60n, 999983n) !== 535869n) throw new Error("case 6");
if (modularPower(1n, 60n, 2n) !== 1n) throw new Error("case 7");
if (modularPower(6n, 5n, 8n) !== 0n) throw new Error("case 8");
if (modularPower(12n, 13n, 17n) !== 14n) throw new Error("case 9");
if (modularPower(5n, 8n, 25n) !== 0n) throw new Error("case 10");
if (modularPower(17n, 11n, 97n) !== 38n) throw new Error("case 11");
if (modularPower(2n, 60n, 99991n) !== 66329n) throw new Error("case 12");
if (modularPower(999n, 3n, 1000n) !== 999n) throw new Error("case 13");
