// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert, assertEquals, assertThrows } from "./test_util.ts";

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
  let err = assertThrows(
    () => {
      localStorage.setItem("k", "v".repeat(15 * 1024 * 1024));
    },
    DOMException,
  );
  assertEquals(err.name, "QuotaExceededError");
  assertEquals(localStorage.getItem("k"), null);
  err = assertThrows(
    () => {
      localStorage.setItem("k".repeat(15 * 1024 * 1024), "v");
    },
    DOMException,
  );
  assertEquals(err.name, "QuotaExceededError");
  err = assertThrows(
    () => {
      localStorage.setItem(
        "k".repeat(5 * 1024 * 1024),
        "v".repeat(5 * 1024 * 1024),
      );
    },
    DOMException,
  );
  assertEquals(err.name, "QuotaExceededError");
});

Deno.test(function webstorageProxy() {
  localStorage.clear();
  localStorage.foo = "foo";
  assertEquals(localStorage.foo, "foo");
  const symbol = Symbol("bar");
  localStorage[symbol as any] = "bar";
  assertEquals(localStorage[symbol as any], "bar");
  assertEquals(symbol in localStorage, true);
});

Deno.test(function webstorageGetOwnPropertyDescriptorSymbol() {
  localStorage.clear();
  Object.getOwnPropertyDescriptor(localStorage, Symbol("foo"));
});
