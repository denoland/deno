import { register } from "node:module";

// Registering a passthrough hook flips on the global `resolve_active`
// flag. Bare-specifier imports from a blob: referrer must still resolve
// via the import map -- a blob: URL is "cannot-be-a-base", so a naive
// async-resolve placeholder would fail to URL-join with the bare
// specifier and surface as "relative URL with a cannot-be-a-base base".
register("../hooks-passthrough.mjs", import.meta.url);

const code = `import { value } from "@mapped/foo";\n` +
  `console.log("blob got:", value);\n`;
const blob = new Blob([code], { type: "text/javascript" });
const url = URL.createObjectURL(blob);
await import(url);
console.log("done");
