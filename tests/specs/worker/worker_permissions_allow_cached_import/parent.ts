// The parent statically imports a remote module, so it becomes part of the
// parent's module graph. The worker is created with `import: false` but
// `allowCachedImport: true`, so it may reuse that already-loaded dependency
// even though it could not import a new remote module itself.
import { add } from "http://localhost:4545/add.ts";

console.log("parent:", add(1, 2));

new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
  deno: {
    permissions: {
      import: false,
    },
    allowCachedImport: true,
  },
});
