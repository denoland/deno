import { foo } from "./timeout_loop.js";
import { collection } from "../circular.js";

console.log("collection in b", collection);
console.log("foo in b", foo);

export const a = "a";
