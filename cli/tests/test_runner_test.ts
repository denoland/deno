// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assert } from "../../std/testing/asserts.ts";

Deno.test({
  name: "never resolve", 
  sanitizeOps: false,
  sanitizeResources: false,
  fn: function () {
    return new Promise((_resolve, _reject) => {
      console.log("in promise");
      // _reject("asdf");
      // Neither `resolve` nor `reject` is called
    });
  }
});

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

window.addEventListener("load", (_e) => {
  console.error("in load handler!");
});

window.addEventListener("unload", (_e) => {
  console.error("in unload handler!");
});