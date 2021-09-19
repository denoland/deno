// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, unitTest } from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function fdatasyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fdatasyncSync.txt";
    const file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    Deno.writeSync(file.rid, data);
    Deno.fdatasyncSync(file.rid);
    assertEquals(Deno.readFileSync(filename), data);
    Deno.close(file.rid);
    Deno.removeSync(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function fdatasyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fdatasync.txt";
    const file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await Deno.write(file.rid, data);
    await Deno.fdatasync(file.rid);
    assertEquals(await Deno.readFile(filename), data);
    Deno.close(file.rid);
    await Deno.remove(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function fsyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fsyncSync.txt";
    const file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    Deno.ftruncateSync(file.rid, size);
    Deno.fsyncSync(file.rid);
    assertEquals(Deno.statSync(filename).size, size);
    Deno.close(file.rid);
    Deno.removeSync(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function fsyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fsync.txt";
    const file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await Deno.ftruncate(file.rid, size);
    await Deno.fsync(file.rid);
    assertEquals((await Deno.stat(filename)).size, size);
    Deno.close(file.rid);
    await Deno.remove(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function fileDatasyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fdatasyncSync.txt";
    const file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    file.writeSync(data);
    file.datasyncSync();

    assertEquals(Deno.readFileSync(filename), data);
    file.close();

    Deno.removeSync(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function fileDatasyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fdatasync.txt";
    const file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await file.write(data);
    await file.datasync();
    assertEquals(await Deno.readFile(filename), data);
    file.close();
    await Deno.remove(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function fileSyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fsyncSync.txt";
    const file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    file.truncateSync(size);
    file.syncSync();
    assertEquals(Deno.statSync(filename).size, size);
    file.close();
    Deno.removeSync(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function fileSyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fsync.txt";
    const file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await file.truncate(size);
    await file.sync();
    assertEquals((await Deno.stat(filename)).size, size);
    Deno.close(file.rid);
    await Deno.remove(filename);
  },
);
