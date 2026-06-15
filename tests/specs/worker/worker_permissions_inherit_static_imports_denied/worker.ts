// This worker opted into `inheritStaticImports`, but it statically imports a
// remote module the parent never loaded into its module graph. Reuse only
// applies to dependencies the parent already resolved, so importing a new
// remote module must still be denied under `import: false`.
import { printHello } from "http://localhost:4545/subdir/print_hello.ts";

console.log("FAIL: import:false worker imported a module the parent had not loaded");
printHello();
