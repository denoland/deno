// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";
import * as deno from "deno";

test(function injectPlatformSuccess() {
  // Make sure they exists and not empty (transformed)
  assert(!!deno.arch);
  assert(!!deno.platform);
});
