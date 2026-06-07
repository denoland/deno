import { getValue, setValue } from "@denotest/esm-basic";

setValue(5);
console.log(getValue());

// The workspace has no root package.json, only this member does. BYONM should
// still be enabled, so a node_modules directory is created at the workspace
// root rather than silently falling back to the global cache (issue #26146).
const rootNodeModules = new URL("../node_modules", import.meta.url);
console.log("root node_modules:", Deno.statSync(rootNodeModules).isDirectory);
