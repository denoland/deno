import fs from "node:fs";
const _data = fs.readFileSync("./node_builtin.js", 123);

// check node:module specifically because for deno check it should
// resolve to the @types/node package, but at runtime it uses a different
// builtin object than deno_std
import { builtinModules } from "node:module";
// should error about being string[]
const _testString: number[] = builtinModules;
