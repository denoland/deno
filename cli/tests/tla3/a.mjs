import { foo } from "./timeout_loop.mjs";

export const collection = [];

const mod = await import("./b.mjs");

console.error("foo in main", foo);
console.error("mod", mod);