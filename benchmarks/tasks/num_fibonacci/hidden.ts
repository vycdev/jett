import { fibonacci } from "./solution.js";

if (fibonacci(0n) !== 0n) throw new Error("case 0");
if (fibonacci(1n) !== 1n) throw new Error("case 1");
if (fibonacci(2n) !== 1n) throw new Error("case 2");
if (fibonacci(3n) !== 2n) throw new Error("case 3");
if (fibonacci(4n) !== 3n) throw new Error("case 4");
if (fibonacci(5n) !== 5n) throw new Error("case 5");
if (fibonacci(8n) !== 21n) throw new Error("case 6");
if (fibonacci(10n) !== 55n) throw new Error("case 7");
if (fibonacci(20n) !== 6765n) throw new Error("case 8");
if (fibonacci(30n) !== 832040n) throw new Error("case 9");
if (fibonacci(40n) !== 102334155n) throw new Error("case 10");
if (fibonacci(50n) !== 12586269025n) throw new Error("case 11");
if (fibonacci(60n) !== 1548008755920n) throw new Error("case 12");
if (fibonacci(70n) !== 190392490709135n) throw new Error("case 13");
