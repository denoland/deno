// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function platformSyncSuccess() {
  const plat = deno.platformSync();
  assert(!!plat.os);
  assert(!!plat.family);
  if (plat.os === "windows") {
    assert(plat.family === "windows");
  } else if (plat.os === "macos" || plat.os === "linux") {
    assert(plat.family === "unix");
  }
});

test(async function platformSuccess() {
  const plat = await deno.platform();
  assert(!!plat.os);
  assert(!!plat.family);
  if (plat.os === "windows") {
    assert(plat.family === "windows");
  } else if (plat.os === "macos" || plat.os === "linux") {
    assert(plat.family === "unix");
  }
});
