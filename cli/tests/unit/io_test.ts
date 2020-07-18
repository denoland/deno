// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals } from "./test_util.ts";

const DEFAULT_BUF_SIZE = 32 * 1024;

type Spy = { calls: number };

function repeat(c: string, bytes: number): Uint8Array {
  assertEquals(c.length, 1);
  const ui8 = new Uint8Array(bytes);
  ui8.fill(c.charCodeAt(0));
  return ui8;
}

function spyRead(obj: Deno.Buffer): Spy {
  const spy: Spy = {
    calls: 0,
  };

  const orig = obj.read.bind(obj);

  obj.read = (p: Uint8Array): Promise<number | null> => {
    spy.calls++;
    return orig(p);
  };

  return spy;
}

unitTest(async function copyWithDefaultBufferSize() {
  const xBytes = repeat("b", DEFAULT_BUF_SIZE);
  const reader = new Deno.Buffer(xBytes.buffer as ArrayBuffer);
  const write = new Deno.Buffer();

  const readSpy = spyRead(reader);

  const n = await Deno.copy(reader, write);

  assertEquals(n, xBytes.length);
  assertEquals(write.length, xBytes.length);
  assertEquals(readSpy.calls, 2); // read with DEFAULT_BUF_SIZE bytes + read with 0 bytes
});

unitTest(async function copyWithCustomBufferSize() {
  const bufSize = 1024;
  const xBytes = repeat("b", DEFAULT_BUF_SIZE);
  const reader = new Deno.Buffer(xBytes.buffer as ArrayBuffer);
  const write = new Deno.Buffer();

  const readSpy = spyRead(reader);

  const n = await Deno.copy(reader, write, { bufSize });

  assertEquals(n, xBytes.length);
  assertEquals(write.length, xBytes.length);
  assertEquals(readSpy.calls, DEFAULT_BUF_SIZE / bufSize + 1);
});

unitTest({ perms: { write: true } }, async function copyBufferToFile() {
  const filePath = "test-file.txt";
  // bigger than max File possible buffer 16kb
  const bufSize = 32 * 1024;
  const xBytes = repeat("b", bufSize);
  const reader = new Deno.Buffer(xBytes.buffer as ArrayBuffer);
  const write = await Deno.open(filePath, { write: true, create: true });

  const n = await Deno.copy(reader, write, { bufSize });

  assertEquals(n, xBytes.length);

  write.close();
  await Deno.remove(filePath);
});
