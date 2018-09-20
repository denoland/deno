// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function transformPlatformSuccess() {
  // Make sure they are transformed
  assert(deno.arch !== "unknown");
  assert(deno.platform !== "unknown");
});
