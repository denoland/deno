export { add } from "./import_inner_inner.js";

Deno.core.print("import_inner.js before\n");

await Deno.core.opAsync("op_sleep");

Deno.core.print("import_inner.js after\n");

