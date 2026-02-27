// Copyright 2018-2025 the Deno authors. MIT license.

import { assertThrows } from "checkin:testing";
const { op_import_sync, op_path_to_url } = Deno.core.ops;

globalThis.onunhandledrejection = (...args) => {
  console.error("unexpected call", args);
};
globalThis.onrejectionhandled = (...args) => {
  console.error("unexpected call", args);
};

assertThrows(
  () => {
    op_import_sync(
      op_path_to_url("./integration/import_sync_throw/module.mjs"),
    );
  },
  Error,
  "this is a test",
);
