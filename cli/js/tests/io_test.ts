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

// TODO(jcao219): This should go into Deno.
function copySync(
  src: Deno.SyncReader,
  dst: Deno.SyncWriter,
  options?: {
    bufSize?: number;
  }
): number {
  let n = 0;
  const bufSize = options?.bufSize ?? DEFAULT_BUF_SIZE;
  const b = new Uint8Array(bufSize);
  while (true) {
    const result = src.readSync(b);
    if (result === Deno.EOF) {
      break;
    } else {
      n += dst.writeSync(b.subarray(0, result));
    }
  }
  return n;
}

function spyReadSync(obj: Deno.Buffer): Spy {
  const spy: Spy = {
    calls: 0,
  };

  const orig = obj.readSync.bind(obj);

  obj.readSync = (p: Uint8Array): number | Deno.EOF => {
    spy.calls++;
    return orig(p);
  };

  return spy;
}

unitTest(async function copyWithDefaultBufferSize() {
  const xBytes = repeat("b", DEFAULT_BUF_SIZE);
  const reader = new Deno.Buffer(xBytes.buffer as ArrayBuffer);
  const write = new Deno.Buffer();

  const readSpy = spyReadSync(reader);

  const n = copySync(reader, write);

  assertEquals(n, xBytes.length);
  assertEquals(write.length, xBytes.length);
  assertEquals(readSpy.calls, 2); // read with DEFAULT_BUF_SIZE bytes + read with 0 bytes
});

unitTest(async function copyWithCustomBufferSize() {
  const bufSize = 1024;
  const xBytes = repeat("b", DEFAULT_BUF_SIZE);
  const reader = new Deno.Buffer(xBytes.buffer as ArrayBuffer);
  const write = new Deno.Buffer();

  const readSpy = spyReadSync(reader);

  const n = copySync(reader, write, { bufSize });

  assertEquals(n, xBytes.length);
  assertEquals(write.length, xBytes.length);
  assertEquals(readSpy.calls, DEFAULT_BUF_SIZE / bufSize + 1);
});
