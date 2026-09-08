import { firstDuplicate } from "./solution.js";

if (firstDuplicate([]) !== null) throw new Error("empty input");
if (firstDuplicate([7n]) !== null) throw new Error("unique input");
if (firstDuplicate([2n, 1n, 3n, 1n, 2n]) !== 1n) throw new Error("second occurrence order");
if (firstDuplicate([4n, 4n, 5n, 5n]) !== 4n) throw new Error("immediate duplicate");
if (firstDuplicate([-2n, 0n, -2n]) !== -2n) throw new Error("negative duplicate");
if (firstDuplicate([0n, 1n, 0n, 1n]) !== 0n) throw new Error("zero duplicate");
