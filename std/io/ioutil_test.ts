// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { Buffer } = Deno;
type Reader = Deno.Reader;
import { assertEquals } from "../testing/asserts.ts";
import {
  copyN,
  readInt,
  readLong,
  readShort,
  sliceLongToBytes,
} from "./ioutil.ts";
import { BufReader } from "./bufio.ts";
import { stringsReader } from "./util.ts";

class BinaryReader implements Reader {
  index = 0;

  constructor(private bytes: Uint8Array = new Uint8Array(0)) {}

  read(p: Uint8Array): Promise<number | Deno.EOF> {
    p.set(this.bytes.subarray(this.index, p.byteLength));
    this.index += p.byteLength;
    return Promise.resolve(p.byteLength);
  }
}

Deno.test(async function testReadShort(): Promise<void> {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34]));
  const short = await readShort(new BufReader(r));
  assertEquals(short, 0x1234);
});

Deno.test(async function testReadInt(): Promise<void> {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34, 0x56, 0x78]));
  const int = await readInt(new BufReader(r));
  assertEquals(int, 0x12345678);
});

Deno.test(async function testReadLong(): Promise<void> {
  const r = new BinaryReader(
    new Uint8Array([0x00, 0x00, 0x00, 0x78, 0x12, 0x34, 0x56, 0x78])
  );
  const long = await readLong(new BufReader(r));
  assertEquals(long, 0x7812345678);
});

Deno.test(async function testReadLong2(): Promise<void> {
  const r = new BinaryReader(
    new Uint8Array([0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78])
  );
  const long = await readLong(new BufReader(r));
  assertEquals(long, 0x12345678);
});

Deno.test(function testSliceLongToBytes(): void {
  const arr = sliceLongToBytes(0x1234567890abcdef);
  const actual = readLong(new BufReader(new BinaryReader(new Uint8Array(arr))));
  const expected = readLong(
    new BufReader(
      new BinaryReader(
        new Uint8Array([0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef])
      )
    )
  );
  assertEquals(actual, expected);
});

Deno.test(function testSliceLongToBytes2(): void {
  const arr = sliceLongToBytes(0x12345678);
  assertEquals(arr, [0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]);
});

Deno.test(async function testCopyN1(): Promise<void> {
  const w = new Buffer();
  const r = stringsReader("abcdefghij");
  const n = await copyN(w, r, 3);
  assertEquals(n, 3);
  assertEquals(w.toString(), "abc");
});

Deno.test(async function testCopyN2(): Promise<void> {
  const w = new Buffer();
  const r = stringsReader("abcdefghij");
  const n = await copyN(w, r, 11);
  assertEquals(n, 10);
  assertEquals(w.toString(), "abcdefghij");
});
