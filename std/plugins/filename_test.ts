// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assert } from "../testing/asserts.ts";
import { pluginFilename } from "./filename.ts";

Deno.test(function filenameWithoutOs() {
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

Deno.test(function filenameWithOs() {
  const filename = "someLib_Name";
  assertEquals(pluginFilename(filename, "linux"), "libsomeLib_Name.so");
  assertEquals(pluginFilename(filename, "mac"), "libsomeLib_Name.dylib");
  assertEquals(pluginFilename(filename, "win"), "someLib_Name.dll");
});
