// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertArrayIncludes, AssertionError, assertThrows } from "./mod.ts";

Deno.test("ArrayContains", function () {
  const fixture = ["deno", "iz", "luv"];
  const fixtureObject = [{ deno: "luv" }, { deno: "Js" }];
  assertArrayIncludes(fixture, ["deno"]);
  assertArrayIncludes(fixtureObject, [{ deno: "luv" }]);
  assertArrayIncludes(
    Uint8Array.from([1, 2, 3, 4]),
    Uint8Array.from([1, 2, 3]),
  );
  assertThrows(
    () => assertArrayIncludes(fixtureObject, [{ deno: "node" }]),
    AssertionError,
    `Expected actual: "[
  {
    deno: "luv",
  },
  {
    deno: "Js",
  },
]" to include: "[
  {
    deno: "node",
  },
]".
missing: [
  {
    deno: "node",
  },
]`,
  );
});
