// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, DENO_FUTURE } from "./test_util.ts";

Deno.test(
  { ignore: DENO_FUTURE, permissions: { read: true, write: true } },
  function fsyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fsyncSync.txt";
    using file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    file.truncateSync(size);
    Deno.fsyncSync(file.rid);
    assertEquals(Deno.statSync(filename).size, size);
    Deno.removeSync(filename);
  },
);

Deno.test(
  { ignore: DENO_FUTURE, permissions: { read: true, write: true } },
  async function fsyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fsync.txt";
    using file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await file.truncate(size);
    await Deno.fsync(file.rid);
    assertEquals((await Deno.stat(filename)).size, size);
    await Deno.remove(filename);
  },
);
