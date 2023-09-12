import version1 from "deno:@denotest/no_module_graph@0.1.0/mod.ts";
import version2 from "deno:@denotest/no_module_graph@^0.2/mod.ts";

console.log(version1);
console.log(version2);
