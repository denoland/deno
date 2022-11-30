// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert, assertFalse } from "./test_util.ts";

Deno.test("isProxy", () => {
  assert(new Proxy({}, {}));

  assertFalse(Deno.isProxy(1));
  assertFalse(Deno.isProxy(""));
  assertFalse(Deno.isProxy([]));
  assertFalse(Deno.isProxy({}));
});
