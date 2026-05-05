import { register } from "node:module";

// Register chain-a first, then chain-b.
// chain-b runs first (LIFO) and short-circuits, so chain-a never sees it.
register("./hooks-chain-a.mjs", import.meta.url);
register("./hooks-chain-b.mjs", import.meta.url);

// Allow hook modules to load
await new Promise((resolve) => setTimeout(resolve, 100));

const { value } = await import("virtual:chain");
console.log(value);
