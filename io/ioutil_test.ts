// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { Buffer, Reader, ReadResult } from "deno";
import { assert, assertEqual, runTests, test } from "../testing/mod.ts";
import {
  copyN,
  readInt,
  readLong,
  readShort,
  sliceLongToBytes
} from "./ioutil.ts";
import { BufReader } from "./bufio.ts";
import { stringsReader } from "./util.ts";

class BinaryReader implements Reader {
  index = 0;

  constructor(private bytes: Uint8Array = new Uint8Array(0)) {}

  async read(p: Uint8Array): Promise<ReadResult> {
    p.set(this.bytes.subarray(this.index, p.byteLength));
    this.index += p.byteLength;
    return { nread: p.byteLength, eof: false };
  }
}

test(async function testReadShort() {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34]));
  const short = await readShort(new BufReader(r));
  assertEqual(short, 0x1234);
});

test(async function testReadInt() {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34, 0x56, 0x78]));
  const int = await readInt(new BufReader(r));
  assertEqual(int, 0x12345678);
});

test(async function testReadLong() {
  const r = new BinaryReader(
    new Uint8Array([0x12, 0x34, 0x56, 0x78, 0x12, 0x34, 0x56, 0x78])
  );
  const long = await readLong(new BufReader(r));
  assertEqual(long, 0x1234567812345678);
});

test(async function testReadLong2() {
  const r = new BinaryReader(
    new Uint8Array([0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78])
  );
  const long = await readLong(new BufReader(r));
  assertEqual(long, 0x12345678);
});

test(async function testSliceLongToBytes() {
  const arr = sliceLongToBytes(0x1234567890abcdef);
  const actual = readLong(new BufReader(new BinaryReader(new Uint8Array(arr))));
  const expected = readLong(
    new BufReader(
      new BinaryReader(
        new Uint8Array([0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef])
      )
    )
  );
  assertEqual(actual, expected);
});

test(async function testSliceLongToBytes2() {
  const arr = sliceLongToBytes(0x12345678);
  assertEqual(arr, [0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]);
});

test(async function testCopyN1() {
  const w = new Buffer();
  const r = stringsReader("abcdefghij");
  const n = await copyN(w, r, 3);
  assert.equal(n, 3);
  assert.equal(w.toString(), "abc");
});

test(async function testCopyN2() {
  const w = new Buffer();
  const r = stringsReader("abcdefghij");
  const n = await copyN(w, r, 11);
  assert.equal(n, 10);
  assert.equal(w.toString(), "abcdefghij");
});
