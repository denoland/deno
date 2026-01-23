// Copyright the Deno authors. MIT license.

import { createRequire } from "node:module";
const require = createRequire(Deno.args[0]);
console.log(require.resolve(Deno.args[1]));