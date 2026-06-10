// Copyright 2018-2026 the Deno authors. MIT license.

import { createRequire } from "node:module";
const require = createRequire(Deno.args[0]);
// deno-lint-ignore no-console
console.log(require.resolve(Deno.args[1]));
