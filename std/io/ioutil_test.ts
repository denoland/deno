// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { BufReader } from "./bufio.ts";
import {
  copyN,
  readInt,
  readLong,
  readShort,
  sliceLongToBytes,
} from "./ioutil.ts";
import { StringReader } from "./readers.ts";

class BinaryReader implements Deno.Reader {
  index = 0;

  constructor(private bytes: Uint8Array = new Uint8Array(0)) {}

  read(p: Uint8Array): Promise<number | null> {
    p.set(this.bytes.subarray(this.index, p.byteLength));
    this.index += p.byteLength;
    return Promise.resolve(p.byteLength);
  }
}

Deno.test("testReadShort", async function (): Promise<void> {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34]));
  const short = await readShort(new BufReader(r));
  assertEquals(short, 0x1234);
});

Deno.test("testReadInt", async function (): Promise<void> {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34, 0x56, 0x78]));
  const int = await readInt(new BufReader(r));
  assertEquals(int, 0x12345678);
});

Deno.test("testReadLong", async function (): Promise<void> {
  const r = new BinaryReader(
    new Uint8Array([0x00, 0x00, 0x00, 0x78, 0x12, 0x34, 0x56, 0x78]),
  );
  const long = await readLong(new BufReader(r));
  assertEquals(long, 0x7812345678);
});

Deno.test("testReadLong2", async function (): Promise<void> {
  const r = new BinaryReader(
    new Uint8Array([0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]),
  );
  const long = await readLong(new BufReader(r));
  assertEquals(long, 0x12345678);
});

Deno.test("testSliceLongToBytes", function (): void {
  const arr = sliceLongToBytes(0x1234567890abcdef);
  const actual = readLong(new BufReader(new BinaryReader(new Uint8Array(arr))));
  const expected = readLong(
    new BufReader(
      new BinaryReader(
        new Uint8Array([0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef]),
      ),
    ),
  );
  assertEquals(actual, expected);
});

Deno.test("testSliceLongToBytes2", function (): void {
  const arr = sliceLongToBytes(0x12345678);
  assertEquals(arr, [0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]);
});

Deno.test("testCopyN1", async function (): Promise<void> {
  const w = new Deno.Buffer();
  const r = new StringReader("abcdefghij");
  const n = await copyN(r, w, 3);
  assertEquals(n, 3);
  assertEquals(new TextDecoder().decode(w.bytes()), "abc");
});

Deno.test("testCopyN2", async function (): Promise<void> {
  const w = new Deno.Buffer();
  const r = new StringReader("abcdefghij");
  const n = await copyN(r, w, 11);
  assertEquals(n, 10);
  assertEquals(new TextDecoder().decode(w.bytes()), "abcdefghij");
});

Deno.test("copyNWriteAllData", async function (): Promise<void> {
  const tmpDir = await Deno.makeTempDir();
  const filepath = `${tmpDir}/data`;
  const file = await Deno.open(filepath, { create: true, write: true });

  const size = 16 * 1024 + 1;
  const data = "a".repeat(32 * 1024);
  const r = new StringReader(data);
  const n = await copyN(r, file, size); // Over max file possible buffer
  file.close();
  await Deno.remove(filepath);

  assertEquals(n, size);
});

Deno.test("testStringReaderEof", async function (): Promise<void> {
  const r = new StringReader("abc");
  assertEquals(await r.read(new Uint8Array()), 0);
  assertEquals(await r.read(new Uint8Array(4)), 3);
  assertEquals(await r.read(new Uint8Array(1)), null);
});
