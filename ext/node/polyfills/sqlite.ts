// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/_sqlite.ts");
export const { backup, DatabaseSync, StatementSync, constants } = _mod;
export default _mod.default;
