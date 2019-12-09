// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals, assert } from "../testing/asserts.ts";
import { pluginFilename } from "./plugin_filename.ts";

test(function filename() {
  const filename = pluginFilename("someLib_Name");
  switch (Deno.build.os) {
    case "linux":
      assertEquals(filename, "libsomeLib_Name.so");
      break;
    case "mac":
      assertEquals(filename, "libsomeLib_Name.dylib");
      break;
    case "win":
      assertEquals(filename, "someLib_Name.dll");
      break;
    default:
      assert(false);
      break;
  }
});
