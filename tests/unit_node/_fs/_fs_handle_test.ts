// Copyright 2018-2025 the Deno authors. MIT license.
import * as path from "@std/path";
import { Buffer } from "node:buffer";
import * as fs from "node:fs/promises";
import { assert, assertEquals } from "@std/assert";

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
