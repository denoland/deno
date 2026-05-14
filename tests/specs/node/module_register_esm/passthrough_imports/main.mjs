import { register } from "node:module";

// Registering a passthrough hook flips on the global "resolve_active" flag
// for the module loader. Bare specifier resolution for subsequent imports
// must still apply the import map (not fall back to URL.parse).
register("../hooks-passthrough.mjs", import.meta.url);

await import("./inner.ts");
console.log("done");
