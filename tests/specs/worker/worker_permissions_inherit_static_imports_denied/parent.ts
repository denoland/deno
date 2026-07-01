// The parent loads `add.ts` into its module graph and spawns a worker with
// `import: false` and `inheritStaticImports: true`. `inheritStaticImports` only
// lets the worker reuse remote modules the parent already loaded, so a
// *different* remote module that the parent never imported must still be denied.
import { add } from "http://localhost:4545/add.ts";

console.log("parent:", add(1, 2));

new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: {
    permissions: {
      import: false,
    },
    inheritStaticImports: true,
  },
});
