// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert, assertThrows } from "./test_util.ts";

Deno.test({ permissions: "none" }, function webStoragesReassignable() {
  // Can reassign to web storages
  globalThis.localStorage = 1 as any;
  globalThis.sessionStorage = 1 as any;
  // The actual values don't change
  assert(globalThis.localStorage instanceof globalThis.Storage);
  assert(globalThis.sessionStorage instanceof globalThis.Storage);
});

Deno.test(function webstorageSizeLimit() {
  localStorage.clear();
  assertThrows(
    () => {
      localStorage.setItem("k", "v".repeat(15 * 1024 * 1024));
    },
    Error,
    "Exceeded maximum storage size",
  );
  assert(localStorage.getItem("k") === null);
  assertThrows(
    () => {
      localStorage.setItem("k".repeat(15 * 1024 * 1024), "v");
    },
    Error,
    "Exceeded maximum storage size",
  );
  assertThrows(
    () => {
      localStorage.setItem(
        "k".repeat(5 * 1024 * 1024),
        "v".repeat(5 * 1024 * 1024),
      );
    },
    Error,
    "Exceeded maximum storage size",
  );
});
