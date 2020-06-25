// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals } from "./test_util.ts";

unitTest(
  {
    ignore: Deno.build.os === "windows",
  },
  function umaskSuccess(): void {
    const prevMask = Deno.umask(0o020);
    const newMask = Deno.umask(prevMask);
    const finalMask = Deno.umask();
    assertEquals(newMask, 0o020);
    assertEquals(finalMask, prevMask);
  }
);
