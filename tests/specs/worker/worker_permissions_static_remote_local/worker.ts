// This module is loaded from a local file, so loading the worker entry itself
// only requires the parent's read access. It then statically imports a remote
// module. The worker was created with `import: false`, so this must be denied
// even though the parent thread has `--allow-import`.
import { add } from "http://localhost:4545/add.ts";

console.log("FAIL: import:false worker was able to import a remote module");
console.log(add(1, 2));
