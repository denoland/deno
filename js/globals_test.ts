// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function globalThisExists() {
  assert(globalThis != null);
});

test(function windowExists() {
  assert(window != null);
});

test(function windowWindowExists() {
  assert(window.window === window);
});

test(function globalThisEqualsWindow() {
  // @ts-ignore (TypeScript thinks globalThis and window don't match)
  assert(globalThis === window);
});

test(function DenoNamespaceExists() {
  assert(Deno != null);
});

test(function DenoNamespaceEqualsWindowDeno() {
  assert(Deno === window.Deno);
});

test(function DenoNamespaceIsFrozen() {
  assert(Object.isFrozen(Deno));
});

test(function webAssemblyExists() {
  assert(typeof WebAssembly.compile === "function");
});
