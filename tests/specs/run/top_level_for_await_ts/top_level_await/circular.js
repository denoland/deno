import { foo } from "./tla3/timeout_loop.js";

export const collection = [];

const mod = await import("./tla3/b.js");

console.log("foo in main", foo);
console.log("mod", mod);
