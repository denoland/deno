// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(async function testErrorClasses() {
  for (const name of deno.errorNames) {
    assert(Object.hasOwnProperty.call(deno, name));
  }
});
