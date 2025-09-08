// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import { DatabaseSync, StatementSync } from "ext:core/ops";

const {
  ObjectDefineProperty,
  SymbolFor,
} = primordials;

export const constants = {
  SQLITE_CHANGESET_OMIT: 0,
  SQLITE_CHANGESET_REPLACE: 1,
  SQLITE_CHANGESET_ABORT: 2,

  SQLITE_CHANGESET_DATA: 1,
  SQLITE_CHANGESET_NOTFOUND: 2,
  SQLITE_CHANGESET_CONFLICT: 3,
  SQLITE_CHANGESET_CONSTRAINT: 4,
  SQLITE_CHANGESET_FOREIGN_KEY: 5,
};

const sqliteTypeSymbol = SymbolFor("sqlite-type");
ObjectDefineProperty(DatabaseSync.prototype, sqliteTypeSymbol, {
  __proto__: null,
  value: "node:sqlite",
  enumerable: false,
  configurable: true,
});

export { DatabaseSync, StatementSync };

export default {
  constants,
  DatabaseSync,
  StatementSync,
};
