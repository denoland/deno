// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(
  {
    ignore: Deno.build.os === "windows",
  },
  function umaskSuccess() {
    const prevMask = Deno.umask(0o020);
    const newMask = Deno.umask(prevMask);
    const finalMask = Deno.umask();
    assertEquals(newMask, 0o020);
    assertEquals(finalMask, prevMask);
  },
);
