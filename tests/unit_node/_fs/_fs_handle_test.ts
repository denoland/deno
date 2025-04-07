// Copyright 2018-2025 the Deno authors. MIT license.
import * as path from "@std/path";
import { Buffer } from "node:buffer";
import * as fs from "node:fs/promises";
import { assert, assertEquals, assertRejects } from "@std/assert";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");
const decoder = new TextDecoder();

Deno.test("readFileSuccess", async function () {
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(decoder.decode(data as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("read", async function () {
  const fileHandle = await fs.open(testData);
  const byteLength = "hello world".length;

  const buf = new Buffer(byteLength);
  await fileHandle.read(buf, 0, byteLength, 0);

  assertEquals(decoder.decode(buf as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("read specify opt", async function () {
  const fileHandle = await fs.open(testData);
  const byteLength = "hello world".length;

  const opt = {
    buffer: new Buffer(byteLength),
    offset: 6,
    length: 5,
    position: 6,
  };
  let res = await fileHandle.read(opt);

  assertEquals(res.bytesRead, 5);
  assertEquals(
    new TextDecoder().decode(res.buffer.subarray(6) as Uint8Array),
    "world",
  );

  const opt2 = {
    buffer: new Buffer(byteLength),
    length: 5,
    position: 0,
  };
  res = await fileHandle.read(opt2);

  assertEquals(res.bytesRead, 5);
  assertEquals(
    decoder.decode(res.buffer.subarray(0, 5) as Uint8Array),
    "hello",
  );

  await fileHandle.close();
});

Deno.test("[node/fs filehandle.write] Write from Buffer", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile, "a+");

  const buffer = Buffer.from("hello world");
  const res = await fileHandle.write(buffer, 0, 5, 0);

  const data = Deno.readFileSync(tempFile);
  await Deno.remove(tempFile);
  await fileHandle.close();

  assertEquals(res.bytesWritten, 5);
  assertEquals(decoder.decode(data), "hello");
});

Deno.test("[node/fs filehandle.write] Write from string", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile, "a+");

  const str = "hello world";
  const res = await fileHandle.write(str);

  const data = Deno.readFileSync(tempFile);
  await Deno.remove(tempFile);
  await fileHandle.close();

  assertEquals(res.bytesWritten, 11);
  assertEquals(decoder.decode(data), "hello world");
});

Deno.test("[node/fs filehandle.stat] Get file status", async function () {
  const fileHandle = await fs.open(testData);
  const stat = await fileHandle.stat();

  assertEquals(stat.isFile(), true);
  assertEquals(stat.size, "hello world".length);

  await fileHandle.close();
});

Deno.test("[node/fs filehandle.writeFile] Write to file", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile, "w");

  const str = "hello world";
  await fileHandle.writeFile(str);

  const data = Deno.readFileSync(tempFile);
  await Deno.remove(tempFile);
  await fileHandle.close();

  assertEquals(decoder.decode(data), "hello world");
});

Deno.test(
  "[node/fs filehandle.writev] Write array of buffers to file",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    const fileHandle = await fs.open(tempFile, "w");

    const buffer1 = Buffer.from("hello ");
    const buffer2 = Buffer.from("world");
    const res = await fileHandle.writev([buffer1, buffer2]);

    const data = Deno.readFileSync(tempFile);
    await Deno.remove(tempFile);
    await fileHandle.close();

    assertEquals(res.bytesWritten, 11);
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test(
  "[node/fs filehandle.writev] Write array of buffers to file with position",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    const fileHandle = await fs.open(tempFile, "w");

    const buffer1 = Buffer.from("hello ");
    const buffer2 = Buffer.from("world");
    await fileHandle.writev([buffer1, buffer2], 0);
    const buffer3 = Buffer.from("lorem ipsum");
    await fileHandle.writev([buffer3], 6);

    const data = Deno.readFileSync(tempFile);
    await Deno.remove(tempFile);
    await fileHandle.close();

    assertEquals(decoder.decode(data), "hello lorem ipsum");
  },
);

Deno.test(
  "[node/fs filehandle.truncate] Truncate file with length",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    const fileHandle = await fs.open(tempFile, "w+");

    await fileHandle.writeFile("hello world");

    await fileHandle.truncate(5);

    const data = Deno.readFileSync(tempFile);
    await Deno.remove(tempFile);
    await fileHandle.close();

    assertEquals(decoder.decode(data), "hello");
  },
);

Deno.test(
  "[node/fs filehandle.truncate] Truncate file without length",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    const fileHandle = await fs.open(tempFile, "w+");

    await fileHandle.writeFile("hello world");

    await fileHandle.truncate();

    const data = Deno.readFileSync(tempFile);
    await Deno.remove(tempFile);
    await fileHandle.close();

    assertEquals(decoder.decode(data), "");
  },
);

Deno.test(
  "[node/fs filehandle.truncate] Truncate file with extension",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    const fileHandle = await fs.open(tempFile, "w+");

    await fileHandle.writeFile("hi");

    await fileHandle.truncate(5);

    const data = Deno.readFileSync(tempFile);
    await Deno.remove(tempFile);
    await fileHandle.close();

    const expected = new Uint8Array(5);
    expected.set(new TextEncoder().encode("hi"));

    assertEquals(data, expected);
    assertEquals(data.length, 5);
    assertEquals(decoder.decode(data.subarray(0, 2)), "hi");
    // Verify null bytes
    assertEquals(data[2], 0);
    assertEquals(data[3], 0);
    assertEquals(data[4], 0);
  },
);

Deno.test(
  "[node/fs filehandle.truncate] Truncate file with negative length",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    const fileHandle = await fs.open(tempFile, "w+");

    await fileHandle.writeFile("hello world");

    await fileHandle.truncate(-1);

    const data = Deno.readFileSync(tempFile);
    await Deno.remove(tempFile);
    await fileHandle.close();

    assertEquals(decoder.decode(data), "");
    assertEquals(data.length, 0);
  },
);

Deno.test({
  name: "[node/fs filehandle.chmod] Change the permissions of the file",
  ignore: Deno.build.os === "windows",
  async fn() {
    const fileHandle = await fs.open(testData);

    const readOnly = 0o444;
    await fileHandle.chmod(readOnly.toString(8));
    assertEquals(Deno.statSync(testData).mode! & 0o777, readOnly);

    const readWrite = 0o666;
    await fileHandle.chmod(readWrite.toString(8));
    assertEquals(Deno.statSync(testData).mode! & 0o777, readWrite);

    await fileHandle.close();
  },
});

Deno.test({
  name:
    "[node/fs filehandle.utimes] Change the file system timestamps of the file",
  async fn() {
    const fileHandle = await fs.open(testData);

    const atime = new Date();
    const mtime = new Date(0);

    await fileHandle.utimes(atime, mtime);
    assertEquals(Deno.statSync(testData).atime!, atime);
    assertEquals(Deno.statSync(testData).mtime!, mtime);

    await fileHandle.close();
  },
});

Deno.test({
  name: "[node/fs filehandle.chown] Change owner of the file",
  ignore: Deno.build.os === "windows",
  async fn() {
    const fileHandle = await fs.open(testData);

    const nobodyUid = 65534;
    const nobodyGid = 65534;

    await assertRejects(
      async () => await fileHandle.chown(nobodyUid, nobodyGid),
      Deno.errors.PermissionDenied,
      "Operation not permitted",
    );

    const realUid = Deno.uid() || 1000;
    const realGid = Deno.gid() || 1000;

    await fileHandle.chown(realUid, realGid);

    assertEquals(Deno.statSync(testData).uid, realUid);
    assertEquals(Deno.statSync(testData).gid, realGid);

    await fileHandle.close();
  },
});

Deno.test({
  name:
    "[node/fs filehandle.sync] Request that all data for the open file descriptor is flushed to the storage device",
  async fn() {
    const fileHandle = await fs.open(testData, "r+");

    await fileHandle.datasync();
    await fileHandle.sync();
    const buf = Buffer.from("hello world");
    await fileHandle.write(buf);
    const ret = await fileHandle.read(Buffer.alloc(11), 0, 11, 0);
    assertEquals(ret.bytesRead, 11);
    assertEquals(ret.buffer, buf);
    await fileHandle.close();
  },
});

Deno.test(
  "[node/fs filehandle.createReadStream] Create a read stream",
  async function () {
    const fileHandle = await fs.open(testData);
    const stream = fileHandle.createReadStream();
    const fileSize = (await fileHandle.stat()).size;

    assertEquals(stream.bytesRead, 0);
    assertEquals(stream.readable, true);

    let bytesRead = 0;

    stream.on("open", () => assertEquals(stream.bytesRead, 0));

    stream.on("data", (data) => {
      assertEquals(data instanceof Buffer, true);
      assertEquals((data as Buffer).byteOffset % 8, 0);
      bytesRead += data.length;
      assertEquals(stream.bytesRead, bytesRead);
    });

    stream.on("end", () => {
      assertEquals(stream.bytesRead, fileSize);
      assertEquals(bytesRead, fileSize);
    });

    // Wait for the 'close' event so that the test won't finish prematurely
    // note: stream automatically closes fd once all the data is read
    await new Promise<void>((resolve) => {
      stream.on("close", resolve);
    });
  },
);

Deno.test(
  "[node/fs filehandle.createWriteStream] Create a write stream",
  async function () {
    const tempFile: string = await Deno.makeTempFile();
    try {
      const fileHandle = await fs.open(tempFile, "w");
      const stream = fileHandle.createWriteStream();
      const { promise, resolve } = Promise.withResolvers<void>();
      stream.on("close", resolve);
      stream.end("a\n", "utf8");
      await promise;
      assertEquals(await Deno.readTextFile(tempFile), "a\n");
    } finally {
      await Deno.remove(tempFile);
    }
  },
);
