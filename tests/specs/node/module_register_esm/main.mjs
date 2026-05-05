import { register } from "node:module";

register("./hooks-basic.mjs", import.meta.url);

// Allow hook module to load before importing
await new Promise((resolve) => setTimeout(resolve, 50));

const { greeting } = await import("virtual:hello");
console.log(greeting);
console.log("done");
