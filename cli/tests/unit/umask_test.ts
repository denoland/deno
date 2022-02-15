// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(
  {
    ignore: Deno.build.os === "windows",
  },
  async function umaskSuccess() {
    const prevMask = Deno.umask(0o023);
    const tmpDir = await Deno.makeTempDir();
    const targetDir = tmpDir + "/foo";
    await Deno.mkdir(targetDir, { mode: 0o777 });
    const stat = await Deno.lstat(targetDir);
    assertEquals(stat.mode! & 0o777, 0o754);
    Deno.umask(prevMask);
  },
);
