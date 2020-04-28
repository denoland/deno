// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This code has been ported almost directly from Go's src/bytes/buffer_test.go
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
import {
  assertEquals,
  assert,
  assertStrContains,
  unitTest,
} from "./test_util.ts";

const { Buffer, readAllSync, writeAllSync } = Deno;
type Buffer = Deno.Buffer;

// N controls how many iterations of certain checks are performed.
const N = 100;
let testBytes: Uint8Array | null;
let testString: string | null;

function init(): void {
  if (testBytes == null) {
    testBytes = new Uint8Array(N);
    for (let i = 0; i < N; i++) {
      testBytes[i] = "a".charCodeAt(0) + (i % 26);
    }
    const decoder = new TextDecoder();
    testString = decoder.decode(testBytes);
  }
}

function check(buf: Deno.Buffer, s: string): void {
  const bytes = buf.bytes();
  assertEquals(buf.length, bytes.byteLength);
  const decoder = new TextDecoder();
  const bytesStr = decoder.decode(bytes);
  assertEquals(bytesStr, s);
  assertEquals(buf.length, buf.toString().length);
  assertEquals(buf.length, s.length);
}

// Fill buf through n writes of byte slice fub.
// The initial contents of buf corresponds to the string s;
// the result is the final contents of buf returned as a string.
function fillBytes(
  buf: Buffer,
  s: string,
  n: number,
  fub: Uint8Array
): string {
  check(buf, s);
  for (; n > 0; n--) {
    const m = buf.writeSync(fub);
    assertEquals(m, fub.byteLength);
    const decoder = new TextDecoder();
    s += decoder.decode(fub);
    check(buf, s);
  }
  return s;
}

// Empty buf through repeated reads into fub.
// The initial contents of buf corresponds to the string s.
function empty(buf: Buffer, s: string, fub: Uint8Array): void {
  check(buf, s);
  while (true) {
    const r = buf.readSync(fub);
    if (r === Deno.EOF) {
      break;
    }
    s = s.slice(r);
    check(buf, s);
  }
  check(buf, "");
}

function repeat(c: string, bytes: number): Uint8Array {
  assertEquals(c.length, 1);
  const ui8 = new Uint8Array(bytes);
  ui8.fill(c.charCodeAt(0));
  return ui8;
}

unitTest(function bufferNewBuffer(): void {
  init();
  assert(testBytes);
  assert(testString);
  const buf = new Buffer(testBytes.buffer as ArrayBuffer);
  check(buf, testString);
});

unitTest(function bufferBasicOperations(): void {
  init();
  assert(testBytes);
  assert(testString);
  const buf = new Buffer();
  for (let i = 0; i < 5; i++) {
    check(buf, "");

    buf.reset();
    check(buf, "");

    buf.truncate(0);
    check(buf, "");

    let n = buf.writeSync(testBytes.subarray(0, 1));
    assertEquals(n, 1);
    check(buf, "a");

    n = buf.writeSync(testBytes.subarray(1, 2));
    assertEquals(n, 1);
    check(buf, "ab");

    n = buf.writeSync(testBytes.subarray(2, 26));
    assertEquals(n, 24);
    check(buf, testString.slice(0, 26));

    buf.truncate(26);
    check(buf, testString.slice(0, 26));

    buf.truncate(20);
    check(buf, testString.slice(0, 20));

    empty(buf, testString.slice(0, 20), new Uint8Array(5));
    empty(buf, "", new Uint8Array(100));

    // TODO buf.writeByte()
    // TODO buf.readByte()
  }
});

unitTest(function bufferReadEmptyAtEOF(): void {
  // check that EOF of 'buf' is not reached (even though it's empty) if
  // results are written to buffer that has 0 length (ie. it can't store any data)
  const buf = new Buffer();
  const zeroLengthTmp = new Uint8Array(0);
  const result = buf.readSync(zeroLengthTmp);
  assertEquals(result, 0);
});

unitTest(function bufferLargeByteWrites(): void {
  init();
  const buf = new Buffer();
  const limit = 9;
  for (let i = 3; i < limit; i += 3) {
    const s = fillBytes(buf, "", 5, testBytes!);
    empty(buf, s, new Uint8Array(Math.floor(testString!.length / i)));
  }
  check(buf, "");
});

unitTest(function bufferTooLargeByteWrites(): void {
  init();
  const tmp = new Uint8Array(72);
  const growLen = Number.MAX_VALUE;
  const xBytes = repeat("x", 0);
  const buf = new Buffer(xBytes.buffer as ArrayBuffer);
  buf.readSync(tmp);

  let err;
  try {
    buf.grow(growLen);
  } catch (e) {
    err = e;
  }

  assert(err instanceof Error);
  assertStrContains(err.message, "grown beyond the maximum size");
});

unitTest(function bufferLargeByteReads(): void {
  init();
  assert(testBytes);
  assert(testString);
  const buf = new Buffer();
  for (let i = 3; i < 30; i += 3) {
    const n = Math.floor(testBytes.byteLength / i);
    const s = fillBytes(buf, "", 5, testBytes.subarray(0, n));
    empty(buf, s, new Uint8Array(testString.length));
  }
  check(buf, "");
});

unitTest(function bufferCapWithPreallocatedSlice(): void {
  const buf = new Buffer(new ArrayBuffer(10));
  assertEquals(buf.capacity, 10);
});

unitTest(function bufferReadFrom(): void {
  init();
  assert(testBytes);
  assert(testString);
  const buf = new Buffer();
  for (let i = 3; i < 30; i += 3) {
    const s = fillBytes(
      buf,
      "",
      5,
      testBytes.subarray(0, Math.floor(testBytes.byteLength / i))
    );
    const b = new Buffer();
    b.readFromSync(buf);
    const fub = new Uint8Array(testString.length);
    empty(b, s, fub);
  }
});

unitTest(function bufferReadFromSync(): void {
  init();
  assert(testBytes);
  assert(testString);
  const buf = new Buffer();
  for (let i = 3; i < 30; i += 3) {
    const s = fillBytes(
      buf,
      "",
      5,
      testBytes.subarray(0, Math.floor(testBytes.byteLength / i))
    );
    const b = new Buffer();
    b.readFromSync(buf);
    const fub = new Uint8Array(testString.length);
    empty(b, s, fub);
  }
});

unitTest(function bufferTestGrow(): void {
  const tmp = new Uint8Array(72);
  for (const startLen of [0, 100, 1000, 10000, 100000]) {
    const xBytes = repeat("x", startLen);
    for (const growLen of [0, 100, 1000, 10000, 100000]) {
      const buf = new Buffer(xBytes.buffer as ArrayBuffer);
      // If we read, this affects buf.off, which is good to test.
      const result = buf.readSync(tmp);
      const nread = result === Deno.EOF ? 0 : result;
      buf.grow(growLen);
      const yBytes = repeat("y", growLen);
      buf.writeSync(yBytes);
      // Check that buffer has correct data.
      assertEquals(
        buf.bytes().subarray(0, startLen - nread),
        xBytes.subarray(nread)
      );
      assertEquals(
        buf.bytes().subarray(startLen - nread, startLen - nread + growLen),
        yBytes
      );
    }
  }
});

unitTest(function testReadAll(): void {
  init();
  assert(testBytes);
  const reader = new Buffer(testBytes.buffer as ArrayBuffer);
  const actualBytes = readAllSync(reader);
  assertEquals(testBytes.byteLength, actualBytes.byteLength);
  for (let i = 0; i < testBytes.length; ++i) {
    assertEquals(testBytes[i], actualBytes[i]);
  }
});

unitTest(function testReadAllSync(): void {
  init();
  assert(testBytes);
  const reader = new Buffer(testBytes.buffer as ArrayBuffer);
  const actualBytes = readAllSync(reader);
  assertEquals(testBytes.byteLength, actualBytes.byteLength);
  for (let i = 0; i < testBytes.length; ++i) {
    assertEquals(testBytes[i], actualBytes[i]);
  }
});

unitTest(function testWriteAll(): void {
  init();
  assert(testBytes);
  const writer = new Buffer();
  writeAllSync(writer, testBytes);
  const actualBytes = writer.bytes();
  assertEquals(testBytes.byteLength, actualBytes.byteLength);
  for (let i = 0; i < testBytes.length; ++i) {
    assertEquals(testBytes[i], actualBytes[i]);
  }
});

unitTest(function testWriteAllSync(): void {
  init();
  assert(testBytes);
  const writer = new Buffer();
  writeAllSync(writer, testBytes);
  const actualBytes = writer.bytes();
  assertEquals(testBytes.byteLength, actualBytes.byteLength);
  for (let i = 0; i < testBytes.length; ++i) {
    assertEquals(testBytes[i], actualBytes[i]);
  }
});
