// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function fdatasyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fdatasyncSync.txt";
    using file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    file.writeSync(data);
    Deno.fdatasyncSync(file.rid);
    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function fdatasyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fdatasync.txt";
    using file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await file.write(data);
    await Deno.fdatasync(file.rid);
    assertEquals(await Deno.readFile(filename), data);
    await Deno.remove(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
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
  { permissions: { read: true, write: true } },
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
