// This worker was created with `import: false`, but the parent already loaded
// this module into its module graph and the worker opted into
// `inheritStaticImports`, so reusing it is allowed.
import { add } from "http://localhost:4545/add.ts";

console.log("worker:", add(3, 4));
self.close();
