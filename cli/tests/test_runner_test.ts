// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assert } from "../../std/testing/asserts.ts";

Deno.test("fail1", function () {
  assert(false, "fail1 assertion");
});

Deno.test("fail2", function () {
  assert(false, "fail2 assertion");
});

Deno.test("success1", function () {
  assert(true);
});

Deno.test("fail3", function () {
  assert(false, "fail3 assertion");
});
