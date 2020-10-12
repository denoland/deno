import { foo } from "./timeout_loop.mjs";
import { collection } from "./a.mjs";

console.error("asdfasdf");
console.log("collection in b", collection);
console.log("foo in b", foo);

export const a = "a";