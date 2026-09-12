import { primeStatus } from "./solution.js";

if (primeStatus(-100n) !== false) throw new Error("case 0");
if (primeStatus(-1n) !== false) throw new Error("case 1");
if (primeStatus(0n) !== false) throw new Error("case 2");
if (primeStatus(1n) !== false) throw new Error("case 3");
if (primeStatus(2n) !== true) throw new Error("case 4");
if (primeStatus(3n) !== true) throw new Error("case 5");
if (primeStatus(4n) !== false) throw new Error("case 6");
if (primeStatus(9n) !== false) throw new Error("case 7");
if (primeStatus(25n) !== false) throw new Error("case 8");
if (primeStatus(49n) !== false) throw new Error("case 9");
if (primeStatus(97n) !== true) throw new Error("case 10");
if (primeStatus(121n) !== false) throw new Error("case 11");
if (primeStatus(997n) !== true) throw new Error("case 12");
if (primeStatus(1024n) !== false) throw new Error("case 13");
if (primeStatus(65521n) !== true) throw new Error("case 14");
if (primeStatus(999983n) !== true) throw new Error("case 15");
if (primeStatus(1000000n) !== false) throw new Error("case 16");
