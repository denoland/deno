// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

// TODO(@kitsonk) remove when we are no longer patching TypeScript to have
// these types available.

Deno.test(function typeCheckingEsNextArrayString() {
  const a = "abcdef";
  assertEquals(a.at(-1), "f");
  const b = ["a", "b", "c", "d", "e", "f"];
  assertEquals(b.at(-1), "f");
  assertEquals(b.findLast((val) => typeof val === "string"), "f");
  assertEquals(b.findLastIndex((val) => typeof val === "string"), 5);
});

Deno.test(function objectHasOwn() {
  const a = { a: 1 };
  assert(Object.hasOwn(a, "a"));
  assert(!Object.hasOwn(a, "b"));
});

Deno.test(function errorCause() {
  const e = new Error("test", { cause: "something" });
  assertEquals(e.cause, "something");
});
