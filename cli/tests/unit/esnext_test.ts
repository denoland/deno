// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, unitTest } from "./test_util.ts";

// TODO(@kitsonk) remove when we are no longer patching TypeScript to have
// these types available.

unitTest(function typeCheckingEsNextArrayString() {
  const a = "abcdef";
  assertEquals(a.at(-1), "f");
  const b = ["a", "b", "c", "d", "e", "f"];
  assertEquals(b.at(-1), "f");
});
