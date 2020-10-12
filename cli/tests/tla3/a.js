import { foo } from "./timeout_loop.js";

export const collection = [];

const mod = await import("./b.js");

console.log("foo in main", foo);
console.log("mod", mod);