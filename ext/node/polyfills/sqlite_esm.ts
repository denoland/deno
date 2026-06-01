// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/sqlite.ts");

export const {
  backup,
  constants,
  DatabaseSync,
  StatementSync,
} = mod;

export default mod;
