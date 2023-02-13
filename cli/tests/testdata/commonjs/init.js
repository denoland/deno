import { fromFileUrl } from "../../../../test_util/std/path/mod.ts";

let moduleName = import.meta.resolve(Deno.args[0]);
moduleName = fromFileUrl(moduleName);

Deno[Deno.internal].node.initialize();
Deno[Deno.internal].require.Module._load(moduleName, null, true);
