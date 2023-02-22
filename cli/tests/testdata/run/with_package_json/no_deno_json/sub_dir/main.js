import * as path from "../../../../../../../test_util/std/path/mod.ts";

const nodeModulesDir = path.join(
  path.dirname(path.dirname(path.fromFileUrl(import.meta.url))),
  "node_modules",
);

console.log(nodeModulesDir);
console.log(Deno.statSync(nodeModulesDir));
