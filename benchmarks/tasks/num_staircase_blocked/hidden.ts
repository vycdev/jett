import { staircaseBlocked } from "./solution.js";

if (staircaseBlocked(0n, []) !== 1n) throw new Error("case 0");
if (staircaseBlocked(1n, []) !== 1n) throw new Error("case 1");
if (staircaseBlocked(1n, [1n]) !== 0n) throw new Error("case 2");
if (staircaseBlocked(2n, [1n]) !== 1n) throw new Error("case 3");
if (staircaseBlocked(2n, [2n]) !== 0n) throw new Error("case 4");
if (staircaseBlocked(3n, []) !== 3n) throw new Error("case 5");
if (staircaseBlocked(4n, [2n]) !== 1n) throw new Error("case 6");
if (staircaseBlocked(5n, [2n, 3n]) !== 0n) throw new Error("case 7");
if (staircaseBlocked(6n, [5n, 1n]) !== 2n) throw new Error("case 8");
if (staircaseBlocked(10n, [4n, 4n, 7n]) !== 6n) throw new Error("case 9");
if (staircaseBlocked(20n, []) !== 10946n) throw new Error("case 10");
if (staircaseBlocked(40n, []) !== 165580141n) throw new Error("case 11");
if (staircaseBlocked(40n, [39n]) !== 63245986n) throw new Error("case 12");
if (staircaseBlocked(8n, [1n, 2n]) !== 0n) throw new Error("case 13");
