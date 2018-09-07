// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(async function testErrorClasses() {
  const num = Object.keys(deno.errorKinds).length / 2;
  for (let i = 0; i < num; ++i) {
    if (i === deno.errorKinds.NoError) {
      continue;
    }
    const kind = deno.errorKinds[i];
    assert(Object.hasOwnProperty.call(deno, `Err${kind}`));
  }
});
