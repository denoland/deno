// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
const {
  backup,
  constants,
  DatabaseSync,
  StatementSync,
} = core.loadExtScript("ext:deno_node/sqlite.ts");

export { backup, constants, DatabaseSync, StatementSync };
export default { backup, constants, DatabaseSync, StatementSync };
