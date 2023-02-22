import * as path from "../../../../../../../test_util/std/path/mod.ts";

const parentDir = path.dirname(path.dirname(path.fromFileUrl(import.meta.url)));

console.log(parentDir);
console.log(Deno.statSync(parentDir));
