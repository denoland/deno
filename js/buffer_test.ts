import { Buffer, readAll } from "deno";
import * as deno from "deno";
// This code has been ported almost directly from Go's src/bytes/buffer_test.go
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
import { assertEqual, test } from "./test_util.ts";
// N controls how many iterations of certain checks are performed.
const N = 100;
let testBytes: Uint8Array | null;
let testString: string | null;

function init() {
  if (testBytes == null) {
    testBytes = new Uint8Array(N);
    for (let i = 0; i < N; i++) {
      testBytes[i] = "a".charCodeAt(0) + (i % 26);
    }
    const decoder = new TextDecoder();
    testString = decoder.decode(testBytes);
  }
}

function check(buf: Buffer, s: string) {
  const bytes = buf.bytes();
  assertEqual(buf.length, bytes.byteLength);
  const decoder = new TextDecoder();
  const bytesStr = decoder.decode(bytes);
  assertEqual(bytesStr, s);
  assertEqual(buf.length, buf.toString().length);
  assertEqual(buf.length, s.length);
}

// Fill buf through n writes of byte slice fub.
// The initial contents of buf corresponds to the string s;
// the result is the final contents of buf returned as a string.
async function fillBytes(
  buf: Buffer,
  s: string,
  n: number,
  fub: Uint8Array
): Promise<string> {
  check(buf, s);
  for (; n > 0; n--) {
    let m = await buf.write(fub);
    assertEqual(m, fub.byteLength);
    const decoder = new TextDecoder();
    s += decoder.decode(fub);
    check(buf, s);
  }
  return s;
}

// Empty buf through repeated reads into fub.
// The initial contents of buf corresponds to the string s.
async function empty(buf: Buffer, s: string, fub: Uint8Array): Promise<void> {
  check(buf, s);
  while (true) {
    const r = await buf.read(fub);
    if (r.nread == 0) {
      break;
    }
    s = s.slice(r.nread);
    check(buf, s);
  }
  check(buf, "");
}

test(function bufferNewBuffer() {
  init();
  const buf = new Buffer(testBytes.buffer as ArrayBuffer);
  check(buf, testString);
});

test(async function bufferBasicOperations() {
  init();
  let buf = new Buffer();
  for (let i = 0; i < 5; i++) {
    check(buf, "");

    buf.reset();
    check(buf, "");

    buf.truncate(0);
    check(buf, "");

    let n = await buf.write(testBytes.subarray(0, 1));
    assertEqual(n, 1);
    check(buf, "a");

    n = await buf.write(testBytes.subarray(1, 2));
    assertEqual(n, 1);
    check(buf, "ab");

    n = await buf.write(testBytes.subarray(2, 26));
    assertEqual(n, 24);
    check(buf, testString.slice(0, 26));

    buf.truncate(26);
    check(buf, testString.slice(0, 26));

    buf.truncate(20);
    check(buf, testString.slice(0, 20));

    await empty(buf, testString.slice(0, 20), new Uint8Array(5));
    await empty(buf, "", new Uint8Array(100));

    // TODO buf.writeByte()
    // TODO buf.readByte()
  }
});

test(async function bufferReadEmptyAtEOF() {
  // check that EOF of 'buf' is not reached (even though it's empty) if
  // results are written to buffer that has 0 length (ie. it can't store any data)
  let buf = new Buffer();
  const zeroLengthTmp = new Uint8Array(0);
  let result = await buf.read(zeroLengthTmp);
  assertEqual(result.nread, 0);
  assertEqual(result.eof, false);
});

test(async function bufferLargeByteWrites() {
  init();
  const buf = new Buffer();
  const limit = 9;
  for (let i = 3; i < limit; i += 3) {
    const s = await fillBytes(buf, "", 5, testBytes);
    await empty(buf, s, new Uint8Array(Math.floor(testString.length / i)));
  }
  check(buf, "");
});

test(async function bufferTooLargeByteWrites() {
  init();
  const tmp = new Uint8Array(72);
  const growLen = Number.MAX_VALUE;
  const xBytes = repeat("x", 0);
  const buf = new Buffer(xBytes.buffer as ArrayBuffer);
  const { nread, eof } = await buf.read(tmp);

  let err;
  try {
    buf.grow(growLen);
  } catch (e) {
    err = e;
  }

  assertEqual(err.kind, deno.ErrorKind.TooLarge);
  assertEqual(err.name, "TooLarge");
});

test(async function bufferLargeByteReads() {
  init();
  const buf = new Buffer();
  for (let i = 3; i < 30; i += 3) {
    const n = Math.floor(testBytes.byteLength / i);
    const s = await fillBytes(buf, "", 5, testBytes.subarray(0, n));
    await empty(buf, s, new Uint8Array(testString.length));
  }
  check(buf, "");
});

test(function bufferCapWithPreallocatedSlice() {
  const buf = new Buffer(new ArrayBuffer(10));
  assertEqual(buf.capacity, 10);
});

test(async function bufferReadFrom() {
  init();
  const buf = new Buffer();
  for (let i = 3; i < 30; i += 3) {
    const s = await fillBytes(
      buf,
      "",
      5,
      testBytes.subarray(0, Math.floor(testBytes.byteLength / i))
    );
    const b = new Buffer();
    await b.readFrom(buf);
    const fub = new Uint8Array(testString.length);
    await empty(b, s, fub);
  }
});

function repeat(c: string, bytes: number): Uint8Array {
  assertEqual(c.length, 1);
  const ui8 = new Uint8Array(bytes);
  ui8.fill(c.charCodeAt(0));
  return ui8;
}

test(async function bufferTestGrow() {
  const tmp = new Uint8Array(72);
  for (let startLen of [0, 100, 1000, 10000, 100000]) {
    const xBytes = repeat("x", startLen);
    for (let growLen of [0, 100, 1000, 10000, 100000]) {
      const buf = new Buffer(xBytes.buffer as ArrayBuffer);
      // If we read, this affects buf.off, which is good to test.
      const { nread, eof } = await buf.read(tmp);
      buf.grow(growLen);
      const yBytes = repeat("y", growLen);
      await buf.write(yBytes);
      // Check that buffer has correct data.
      assertEqual(
        buf.bytes().subarray(0, startLen - nread),
        xBytes.subarray(nread)
      );
      assertEqual(
        buf.bytes().subarray(startLen - nread, startLen - nread + growLen),
        yBytes
      );
    }
  }
});

test(async function testReadAll() {
  init();
  const reader = new Buffer(testBytes.buffer as ArrayBuffer);
  const actualBytes = await readAll(reader);
  assertEqual(testBytes.byteLength, actualBytes.byteLength);
  for (let i = 0; i < testBytes.length; ++i) {
    assertEqual(testBytes[i], actualBytes[i]);
  }
});
