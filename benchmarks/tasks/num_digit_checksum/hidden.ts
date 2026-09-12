import { digitChecksum } from "./solution.js";

if (digitChecksum(0n) !== 0n) throw new Error("case 0");
if (digitChecksum(1n) !== 1n) throw new Error("case 1");
if (digitChecksum(9n) !== 9n) throw new Error("case 2");
if (digitChecksum(10n) !== -1n) throw new Error("case 3");
if (digitChecksum(11n) !== 0n) throw new Error("case 4");
if (digitChecksum(12n) !== 1n) throw new Error("case 5");
if (digitChecksum(123n) !== 2n) throw new Error("case 6");
if (digitChecksum(1234n) !== 2n) throw new Error("case 7");
if (digitChecksum(90909n) !== 27n) throw new Error("case 8");
if (digitChecksum(100001n) !== 0n) throw new Error("case 9");
if (digitChecksum(987654321n) !== 5n) throw new Error("case 10");
if (digitChecksum(1000000000000n) !== 1n) throw new Error("case 11");
if (digitChecksum(999999999999n) !== 0n) throw new Error("case 12");
if (digitChecksum(10101010101n) !== 6n) throw new Error("case 13");
