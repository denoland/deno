import { foo } from "./timeout_loop.js";
import { collection } from "../top_level_await_circular.js";

console.log("collection in b", collection);
console.log("foo in b", foo);

export const a = "a";
