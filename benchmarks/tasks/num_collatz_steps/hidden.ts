import { collatzSteps } from "./solution.js";

if (collatzSteps(1n, 0n) !== 0n) throw new Error("case 0");
if (collatzSteps(2n, 0n) !== null) throw new Error("case 1");
if (collatzSteps(2n, 1n) !== 1n) throw new Error("case 2");
if (collatzSteps(3n, 6n) !== null) throw new Error("case 3");
if (collatzSteps(3n, 7n) !== 7n) throw new Error("case 4");
if (collatzSteps(6n, 8n) !== 8n) throw new Error("case 5");
if (collatzSteps(7n, 15n) !== null) throw new Error("case 6");
if (collatzSteps(7n, 16n) !== 16n) throw new Error("case 7");
if (collatzSteps(27n, 110n) !== null) throw new Error("case 8");
if (collatzSteps(27n, 111n) !== 111n) throw new Error("case 9");
if (collatzSteps(1000000n, 200n) !== 152n) throw new Error("case 10");
if (collatzSteps(999999n, 200n) !== null) throw new Error("case 11");
if (collatzSteps(1024n, 9n) !== null) throw new Error("case 12");
if (collatzSteps(1024n, 10n) !== 10n) throw new Error("case 13");
