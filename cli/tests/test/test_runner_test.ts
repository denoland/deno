// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assert } from "../../../test_util/std/testing/asserts.ts";

setTimeout(function () {
  // This timeout isn't expected to be actually called and is just here keep
  // the event loop alive and ensure that the test runner closes once all tests
  // have run even if there are pending promises.
}, 3600 * 1000);

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
