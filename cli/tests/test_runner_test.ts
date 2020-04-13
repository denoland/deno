// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assert } from "../../std/testing/asserts.ts";

Deno.test(function fail1() {
  assert(false, "fail1 assertion");
});

Deno.test(function fail2() {
  assert(false, "fail2 assertion");
});

Deno.test(function success1() {
  assert(true);
});

Deno.test(function fail3() {
  assert(false, "fail3 assertion");
});
