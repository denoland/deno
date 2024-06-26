// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { Buffer } from "node:buffer";
import { assertEquals, assertThrows } from "@std/assert/mod.ts";

Deno.test({
  name: "[node/buffer] alloc fails if size is not a number",
  fn() {
    const invalidSizes = [{}, "1", "foo", []];

    for (const size of invalidSizes) {
      assertThrows(
        () => {
          // deno-lint-ignore ban-ts-comment
          // @ts-expect-error
          Buffer.alloc(size);
        },
        TypeError,
        '"size" argument must be of type number',
        "should throw on non-number size",
      );
    }
  },
});

Deno.test({
  name: "[node/buffer] alloc allocates a buffer with the expected size",
  fn() {
    const buffer: Buffer = Buffer.alloc(1);
    assertEquals(buffer.length, 1, "Buffer size should be 1");
    assertEquals(buffer[0], 0, "Content should be filled with 0");
  },
});

Deno.test({
  name: "[node/buffer] alloc(0) creates an empty buffer",
  fn() {
    const buffer: Buffer = Buffer.alloc(0);
    assertEquals(buffer.length, 0, "Buffer size should be 0");
  },
});

Deno.test({
  name: "[node/buffer] allocUnsafe allocates a buffer with the expected size",
  fn() {
    const buffer: Buffer = Buffer.allocUnsafe(1);
    assertEquals(buffer.length, 1, "Buffer size should be 1");
  },
});

Deno.test({
  name: "[node/buffer] allocUnsafe(0) creates an empty buffer",
  fn() {
    const buffer: Buffer = Buffer.allocUnsafe(0);
    assertEquals(buffer.length, 0, "Buffer size should be 0");
  },
});

Deno.test({
  name: "[node/buffer] alloc filled correctly with integer",
  fn() {
    const buffer: Buffer = Buffer.alloc(3, 5);
    assertEquals(buffer, Buffer.from([5, 5, 5]));
  },
});

Deno.test({
  name: "[node/buffer] alloc filled correctly with single character",
  fn() {
    assertEquals(Buffer.alloc(5, "a"), Buffer.from([97, 97, 97, 97, 97]));
  },
});

Deno.test({
  name: "[node/buffer] alloc filled correctly with base64 string",
  fn() {
    assertEquals(
      Buffer.alloc(11, "aGVsbG8gd29ybGQ=", "base64"),
      Buffer.from([104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]),
    );
  },
});

Deno.test({
  name: "[node/buffer] alloc filled correctly with hex string",
  fn() {
    assertEquals(
      Buffer.alloc(4, "64656e6f", "hex"),
      Buffer.from([100, 101, 110, 111]),
    );
  },
});

Deno.test({
  name:
    "[node/buffer] alloc filled correctly with hex string smaller than alloc size",
  fn() {
    assertEquals(
      Buffer.alloc(13, "64656e6f", "hex").toString(),
      "denodenodenod",
    );
  },
});

Deno.test({
  name:
    "[node/buffer] alloc filled correctly with Uint8Array smaller than alloc size",
  fn() {
    // todo(fbaltor): remove this 'any' when @types/node fixes https://github.com/DefinitelyTyped/DefinitelyTyped/discussions/65831
    // deno-lint-ignore no-explicit-any
    const arr: any = new Uint8Array([100, 101]);
    assertEquals(
      Buffer.alloc(7, arr),
      Buffer.from([100, 101, 100, 101, 100, 101, 100]),
    );
    assertEquals(
      Buffer.alloc(6, arr),
      Buffer.from([100, 101, 100, 101, 100, 101]),
    );
  },
});

Deno.test({
  name:
    "[node/buffer] alloc filled correctly with Uint8Array bigger than alloc size",
  fn() {
    assertEquals(
      // todo(fbaltor): remove this 'any' when @types/node fixes https://github.com/DefinitelyTyped/DefinitelyTyped/discussions/65831
      // deno-lint-ignore no-explicit-any
      Buffer.alloc(1, new Uint8Array([100, 101]) as any),
      Buffer.from([100]),
    );
  },
});

Deno.test({
  name: "[node/buffer] alloc filled correctly with Buffer",
  fn() {
    assertEquals(
      Buffer.alloc(6, Buffer.from([100, 101])),
      Buffer.from([100, 101, 100, 101, 100, 101]),
    );
    assertEquals(
      Buffer.alloc(7, Buffer.from([100, 101])),
      Buffer.from([100, 101, 100, 101, 100, 101, 100]),
    );
  },
});

// tests from:
// https://github.com/nodejs/node/blob/56dbe466fdbc598baea3bfce289bf52b97b8b8f7/test/parallel/test-buffer-bytelength.js#L70
Deno.test({
  name: "[node/buffer] Byte length is the expected for strings",
  fn() {
    // Special case: zero length string
    assertEquals(Buffer.byteLength("", "ascii"), 0);
    // deno-lint-ignore no-explicit-any
    assertEquals(Buffer.byteLength("", "HeX" as any), 0);

    // utf8
    assertEquals(Buffer.byteLength("∑éllö wørl∂!", "utf-8"), 19);
    assertEquals(Buffer.byteLength("κλμνξο", "utf8"), 12);
    assertEquals(Buffer.byteLength("挵挶挷挸挹", "utf-8"), 15);
    // deno-lint-ignore no-explicit-any
    assertEquals(Buffer.byteLength("𠝹𠱓𠱸", "UTF8" as any), 12);
    // Without an encoding, utf8 should be assumed
    assertEquals(Buffer.byteLength("hey there"), 9);
    assertEquals(Buffer.byteLength("𠱸挶νξ#xx :)"), 17);
    // deno-lint-ignore no-explicit-any
    assertEquals(Buffer.byteLength("hello world", "" as any), 11);
    // It should also be assumed with unrecognized encoding
    // deno-lint-ignore no-explicit-any
    assertEquals(Buffer.byteLength("hello world", "abc" as any), 11);
    // deno-lint-ignore no-explicit-any
    assertEquals(Buffer.byteLength("ßœ∑≈", "unkn0wn enc0ding" as any), 10);

    // base64
    assertEquals(Buffer.byteLength("aGVsbG8gd29ybGQ=", "base64"), 11);
    // deno-lint-ignore no-explicit-any
    assertEquals(Buffer.byteLength("aGVsbG8gd29ybGQ=", "BASE64" as any), 11);
    assertEquals(Buffer.byteLength("bm9kZS5qcyByb2NrcyE=", "base64"), 14);
    assertEquals(Buffer.byteLength("aGkk", "base64"), 3);
    assertEquals(
      Buffer.byteLength("bHNrZGZsa3NqZmtsc2xrZmFqc2RsZmtqcw==", "base64"),
      25,
    );
    // special padding
    assertEquals(Buffer.byteLength("aaa=", "base64"), 2);
    assertEquals(Buffer.byteLength("aaaa==", "base64"), 3);

    assertEquals(Buffer.byteLength("Il était tué"), 14);
    assertEquals(Buffer.byteLength("Il était tué", "utf8"), 14);

    ["ascii", "latin1", "binary"]
      .reduce((es: string[], e: string) => es.concat(e, e.toUpperCase()), [])
      .forEach((encoding: string) => {
        // deno-lint-ignore no-explicit-any
        assertEquals(Buffer.byteLength("Il était tué", encoding as any), 12);
      });

    ["ucs2", "ucs-2", "utf16le", "utf-16le"]
      .reduce((es: string[], e: string) => es.concat(e, e.toUpperCase()), [])
      .forEach((encoding: string) => {
        // deno-lint-ignore no-explicit-any
        assertEquals(Buffer.byteLength("Il était tué", encoding as any), 24);
      });
  },
});

Deno.test({
  name: "[node/buffer] Byte length is the expected one for non-strings",
  fn() {
    assertEquals(
      Buffer.byteLength(Buffer.alloc(0)),
      Buffer.alloc(0).byteLength,
      "Byte length differs on buffers",
    );
  },
});

Deno.test({
  name: "[node/buffer] Two Buffers are concatenated",
  fn() {
    const data1 = [1, 2, 3];
    const data2 = [4, 5, 6];

    const buffer1 = Buffer.from(data1);
    const buffer2 = Buffer.from(data2);

    const resultBuffer = Buffer.concat([buffer1, buffer2]);
    const expectedBuffer = Buffer.from([...data1, ...data2]);
    assertEquals(resultBuffer, expectedBuffer);
  },
});

Deno.test({
  name: "[node/buffer] A single buffer concatenates and return the same buffer",
  fn() {
    const buffer1 = Buffer.alloc(1);
    const resultBuffer = Buffer.concat([buffer1]);
    assertEquals(resultBuffer.length, 1, "Buffer length should be 1");
  },
});

Deno.test({
  name: "[node/buffer] No buffers concat returns an empty buffer",
  fn() {
    const resultBuffer = Buffer.concat([]);
    assertEquals(resultBuffer.length, 0, "Buffer length should be 0");
  },
});

Deno.test({
  name: "[node/buffer] Buffer concat respects totalLength parameter",
  fn() {
    const maxLength1 = 10;
    const buffer1 = Buffer.alloc(2);
    const buffer2 = Buffer.alloc(2);
    assertEquals(
      Buffer.concat([buffer1, buffer2], maxLength1).length,
      maxLength1,
    );

    const maxLength2 = 3;
    const buffer3 = Buffer.alloc(2);
    const buffer4 = Buffer.alloc(2);
    assertEquals(
      Buffer.concat([buffer3, buffer4], maxLength2).length,
      maxLength2,
    );
  },
});

Deno.test({
  name: "[node/buffer] Buffer 8 bit unsigned integers",
  fn() {
    const buffer = Buffer.from([0xff, 0x2a, 0x2a, 0x2a]);
    assertEquals(buffer.readUInt8(0), 255);
    assertEquals(buffer.readUInt8(1), 42);
    assertEquals(buffer.readUInt8(2), 42);
    assertEquals(buffer.readUInt8(3), 42);
  },
});

Deno.test({
  name: "[node/buffer] Buffer 16 bit unsigned integers",
  fn() {
    const buffer = Buffer.from([0x00, 0x2a, 0x42, 0x3f]);
    assertEquals(buffer.readUInt16BE(0), 0x2a);
    assertEquals(buffer.readUInt16BE(1), 0x2a42);
    assertEquals(buffer.readUInt16BE(2), 0x423f);
    assertEquals(buffer.readUInt16LE(0), 0x2a00);
    assertEquals(buffer.readUInt16LE(1), 0x422a);
    assertEquals(buffer.readUInt16LE(2), 0x3f42);

    buffer[0] = 0xfe;
    buffer[1] = 0xfe;
    assertEquals(buffer.readUInt16BE(0), 0xfefe);
    assertEquals(buffer.readUInt16LE(0), 0xfefe);
  },
});

Deno.test({
  name: "[node/buffer] Buffer 32 bit unsigned integers",
  fn() {
    const buffer = Buffer.from([0x32, 0x65, 0x42, 0x56, 0x23, 0xff]);
    assertEquals(buffer.readUInt32BE(0), 0x32654256);
    assertEquals(buffer.readUInt32BE(1), 0x65425623);
    assertEquals(buffer.readUInt32BE(2), 0x425623ff);
    assertEquals(buffer.readUInt32LE(0), 0x56426532);
    assertEquals(buffer.readUInt32LE(1), 0x23564265);
    assertEquals(buffer.readUInt32LE(2), 0xff235642);
  },
});

Deno.test({
  name: "[node/buffer] Buffer readUIntBE",
  fn() {
    const buffer = Buffer.from([
      0x01,
      0x02,
      0x03,
      0x04,
      0x05,
      0x06,
      0x07,
      0x08,
    ]);
    assertEquals(buffer.readUIntBE(0, 1), 0x01);
    assertEquals(buffer.readUIntBE(0, 2), 0x0102);
    assertEquals(buffer.readUIntBE(0, 4), 0x01020304);
  },
});

Deno.test({
  name: "[node/buffer] Buffer readUIntLE",
  fn() {
    const buffer = Buffer.from([
      0x01,
      0x02,
      0x03,
      0x04,
      0x05,
      0x06,
      0x07,
      0x08,
    ]);
    assertEquals(buffer.readUIntLE(0, 1), 0x01);
    assertEquals(buffer.readUIntLE(0, 2), 0x0201);
    assertEquals(buffer.readUIntLE(0, 4), 0x04030201);
  },
});

Deno.test({
  name: "[node/buffer] Buffer copy works as expected",
  fn() {
    const data1 = new Uint8Array([1, 2, 3]);
    const data2 = new Uint8Array([4, 5, 6]);

    const buffer1 = Buffer.from(data1);
    const buffer2 = Buffer.from(data2);

    //Mutates data_1
    data1.set(data2);
    //Mutates buffer_1
    buffer2.copy(buffer1);

    assertEquals(
      Buffer.from(data1),
      buffer1,
    );
  },
});

Deno.test({
  name: "[node/buffer] Buffer copy respects the starting point for copy",
  fn() {
    const buffer1 = Buffer.from([1, 2, 3]);
    const buffer2 = Buffer.alloc(8);

    buffer1.copy(buffer2, 5);

    const expected = Buffer.from([0, 0, 0, 0, 0, 1, 2, 3]);

    assertEquals(
      buffer2,
      expected,
    );
  },
});

Deno.test({
  name:
    "[node/buffer] Buffer copy doesn't throw on offset but copies until offset reached",
  fn() {
    const buffer1 = Buffer.from([1, 2, 3]);
    const buffer2 = Buffer.alloc(8);

    const writtenBytes1 = buffer1.copy(buffer2, 6);

    assertEquals(
      writtenBytes1,
      2,
    );

    assertEquals(
      buffer2,
      Buffer.from([0, 0, 0, 0, 0, 0, 1, 2]),
    );

    const buffer3 = Buffer.from([1, 2, 3]);
    const buffer4 = Buffer.alloc(8);

    const writtenBytes2 = buffer3.copy(buffer4, 8);

    assertEquals(
      writtenBytes2,
      0,
    );

    assertEquals(
      buffer4,
      Buffer.from([0, 0, 0, 0, 0, 0, 0, 0]),
    );
  },
});

Deno.test({
  name: "[node/buffer] Buffer from string creates a Buffer",
  fn() {
    const buffer: Buffer = Buffer.from("test");
    assertEquals(buffer.length, 4, "Buffer length should be 4");
    assertEquals(
      buffer.toString(),
      "test",
      "Buffer to string should recover the string",
    );
  },
});

Deno.test({
  name: "[node/buffer] Buffer from string hex",
  fn() {
    for (const encoding of ["hex", "HEX"]) {
      const buffer: Buffer = Buffer.from(
        "7468697320697320612074c3a97374",
        // deno-lint-ignore no-explicit-any
        encoding as any,
      );
      assertEquals(buffer.length, 15, "Buffer length should be 15");
      assertEquals(
        buffer.toString(),
        "this is a tést",
        "Buffer to string should recover the string",
      );
    }
  },
});

Deno.test({
  name: "[node/buffer] Buffer from string base64",
  fn() {
    for (const encoding of ["base64", "BASE64"]) {
      const buffer: Buffer = Buffer.from(
        "dGhpcyBpcyBhIHTDqXN0",
        // deno-lint-ignore no-explicit-any
        encoding as any,
      );
      assertEquals(buffer.length, 15, "Buffer length should be 15");
      assertEquals(
        buffer.toString(),
        "this is a tést",
        "Buffer to string should recover the string",
      );
    }
  },
});

Deno.test({
  name: "[node/buffer] Buffer to string base64",
  fn() {
    for (const encoding of ["base64", "BASE64"]) {
      const buffer: Buffer = Buffer.from("deno land");
      assertEquals(
        // deno-lint-ignore no-explicit-any
        buffer.toString(encoding as any),
        "ZGVubyBsYW5k",
        "Buffer to string should recover the string in base64",
      );
    }
    const b64 = "dGhpcyBpcyBhIHTDqXN0";
    assertEquals(Buffer.from(b64, "base64").toString("base64"), b64);
  },
});

Deno.test({
  name: "[node/buffer] Buffer to string hex",
  fn() {
    for (const encoding of ["hex", "HEX"]) {
      const buffer: Buffer = Buffer.from("deno land");
      assertEquals(
        // deno-lint-ignore no-explicit-any
        buffer.toString(encoding as any),
        "64656e6f206c616e64",
        "Buffer to string should recover the string",
      );
    }
    const hex = "64656e6f206c616e64";
    assertEquals(Buffer.from(hex, "hex").toString("hex"), hex);
  },
});

Deno.test({
  name: "[node/buffer] Buffer from string invalid encoding",
  fn() {
    const defaultToUtf8Encodings = [null, 5, {}, true, false, ""];
    const invalidEncodings = ["deno", "base645"];

    for (const encoding of defaultToUtf8Encodings) {
      // deno-lint-ignore no-explicit-any
      assertEquals(Buffer.from("yes", encoding as any).toString(), "yes");
    }

    for (const encoding of invalidEncodings) {
      assertThrows(
        () => {
          // deno-lint-ignore no-explicit-any
          Buffer.from("yes", encoding as any);
        },
        TypeError,
        `Unknown encoding: ${encoding}`,
      );
    }
  },
});

Deno.test({
  name: "[node/buffer] Buffer from another buffer creates a copy",
  fn() {
    const buffer1: Buffer = Buffer.from("test");
    const buffer2: Buffer = Buffer.from(buffer1);
    assertEquals(buffer2.length, 4, "Buffer length should be 4");
    assertEquals(
      buffer2.toString(),
      "test",
      "Buffer to string should recover the string",
    );
    buffer1[0] = 114;
    assertEquals(buffer2.toString(), "test", "Buffer should be a copy");
  },
});

Deno.test({
  name: "[node/buffer] isBuffer returns true if the object is a buffer",
  fn() {
    assertEquals(Buffer.isBuffer(Buffer.from("test")), true);
  },
});

Deno.test({
  name: "[node/buffer] isBuffer returns false if the object is not a buffer",
  fn() {
    assertEquals(Buffer.isBuffer({ test: 3 }), false);
    assertEquals(Buffer.isBuffer(new Uint8Array()), false);
  },
});

Deno.test({
  name: "[node/buffer] Buffer toJSON",
  fn() {
    assertEquals(
      JSON.stringify(Buffer.from("deno")),
      '{"type":"Buffer","data":[100,101,110,111]}',
    );
  },
});

Deno.test({
  name: "[node/buffer] slice does not create a copy",
  fn() {
    const buf = Buffer.from("ceno");
    // This method is not compatible with the Uint8Array.prototype.slice()
    const slice = buf.slice();
    slice[0]++;
    assertEquals(slice.toString(), "deno");
  },
});

Deno.test({
  name: "[node/buffer] slice with infinity returns empty buffer",
  fn() {
    const buf = Buffer.from([1, 2, 3, 4, 5]);
    assertEquals(buf.slice(Infinity).length, 0);
  },
});

Deno.test({
  name: "[node/buffer] isEncoding returns true for valid encodings",
  fn() {
    [
      "hex",
      "HEX",
      "HeX",
      "utf8",
      "utf-8",
      "ascii",
      "latin1",
      "binary",
      "base64",
      "BASE64",
      "BASe64",
      "ucs2",
      "ucs-2",
      "utf16le",
      "utf-16le",
    ].forEach((enc) => {
      assertEquals(Buffer.isEncoding(enc), true);
    });
  },
});

Deno.test({
  name: "[node/buffer] isEncoding returns false for invalid encodings",
  fn() {
    [
      "utf9",
      "utf-7",
      "Unicode-FTW",
      "new gnu gun",
      false,
      NaN,
      {},
      Infinity,
      [],
      1,
      0,
      -1,
    ].forEach((enc) => {
      // @ts-expect-error This deliberately ignores the type constraint
      assertEquals(Buffer.isEncoding(enc), false);
    });
  },
});

Deno.test({
  name:
    "[node/buffer] utf8Write handle missing optional length argument (https://github.com/denoland/deno_std/issues/2046)",
  fn() {
    const buf = Buffer.alloc(8);
    // @ts-expect-error Buffer.prototype.utf8Write is an undocumented API
    assertEquals(buf.utf8Write("abc", 0), 3);
    assertEquals([...buf], [0x61, 0x62, 0x63, 0, 0, 0, 0, 0]);
  },
});
