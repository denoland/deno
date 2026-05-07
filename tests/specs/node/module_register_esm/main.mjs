import { register } from "node:module";

register("./hooks-basic.mjs", import.meta.url);

const { greeting } = await import("virtual:hello");
console.log(greeting);
console.log("done");
