// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../testing/asserts.ts";
import { Buffer } from "./buffer.ts";

Deno.test({
  name: "alloc fails on negative numbers",
  fn() {
    assertThrows(
      () => {
        Buffer.alloc(-1);
      },
      RangeError,
      "Invalid typed array length: -1",
      "should throw on negative numbers",
    );
  },
});

Deno.test({
  name: "alloc fails if size is not a number",
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
        `The "size" argument must be of type number. Received type ${typeof size}`,
        "should throw on non-number size",
      );
    }
  },
});

Deno.test({
  name: "alloc(>0) fails if value is an empty Buffer/Uint8Array",
  fn() {
    const invalidValues = [new Uint8Array(), Buffer.alloc(0)];

    for (const value of invalidValues) {
      assertThrows(
        () => {
          console.log(value.constructor.name);
          Buffer.alloc(1, value);
        },
        TypeError,
        `The argument "value" is invalid. Received ${value.constructor.name} []`,
        "should throw for empty Buffer/Uint8Array",
      );
    }
  },
});

Deno.test({
  name: "alloc(0) doesn't fail if value is an empty Buffer/Uint8Array",
  fn() {
    const invalidValues = [new Uint8Array(), Buffer.alloc(0)];

    for (const value of invalidValues) {
      assertEquals(Buffer.alloc(0, value).length, 0);
    }
  },
});

Deno.test({
  name: "alloc allocates a buffer with the expected size",
  fn() {
    const buffer: Buffer = Buffer.alloc(1);
    assertEquals(buffer.length, 1, "Buffer size should be 1");
    assertEquals(buffer[0], 0, "Content should be filled with 0");
  },
});

Deno.test({
  name: "alloc(0) creates an empty buffer",
  fn() {
    const buffer: Buffer = Buffer.alloc(0);
    assertEquals(buffer.length, 0, "Buffer size should be 0");
  },
});

Deno.test({
  name: "allocUnsafe allocates a buffer with the expected size",
  fn() {
    const buffer: Buffer = Buffer.allocUnsafe(1);
    assertEquals(buffer.length, 1, "Buffer size should be 1");
  },
});

Deno.test({
  name: "allocUnsafe(0) creates an empty buffer",
  fn() {
    const buffer: Buffer = Buffer.allocUnsafe(0);
    assertEquals(buffer.length, 0, "Buffer size should be 0");
  },
});

Deno.test({
  name: "alloc filled correctly with integer",
  fn() {
    const buffer: Buffer = Buffer.alloc(3, 5);
    assertEquals(buffer, new Uint8Array([5, 5, 5]));
  },
});

Deno.test({
  name: "alloc filled correctly with single character",
  fn() {
    assertEquals(Buffer.alloc(5, "a"), new Uint8Array([97, 97, 97, 97, 97]));
  },
});

Deno.test({
  name: "alloc filled correctly with base64 string",
  fn() {
    assertEquals(
      Buffer.alloc(11, "aGVsbG8gd29ybGQ=", "base64"),
      new Uint8Array([104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]),
    );
  },
});

Deno.test({
  name: "alloc filled correctly with hex string",
  fn() {
    assertEquals(
      Buffer.alloc(4, "64656e6f", "hex"),
      new Uint8Array([100, 101, 110, 111]),
    );
  },
});

Deno.test({
  name: "alloc filled correctly with hex string smaller than alloc size",
  fn() {
    assertEquals(
      Buffer.alloc(13, "64656e6f", "hex").toString(),
      "denodenodenod",
    );
  },
});

Deno.test({
  name: "alloc filled correctly with Uint8Array smaller than alloc size",
  fn() {
    assertEquals(
      Buffer.alloc(7, new Uint8Array([100, 101])),
      new Uint8Array([100, 101, 100, 101, 100, 101, 100]),
    );
    assertEquals(
      Buffer.alloc(6, new Uint8Array([100, 101])),
      new Uint8Array([100, 101, 100, 101, 100, 101]),
    );
  },
});

Deno.test({
  name: "alloc filled correctly with Uint8Array bigger than alloc size",
  fn() {
    assertEquals(
      Buffer.alloc(1, new Uint8Array([100, 101])),
      new Uint8Array([100]),
    );
  },
});

Deno.test({
  name: "alloc filled correctly with Buffer",
  fn() {
    assertEquals(
      Buffer.alloc(6, new Buffer([100, 101])),
      new Uint8Array([100, 101, 100, 101, 100, 101]),
    );
    assertEquals(
      Buffer.alloc(7, new Buffer([100, 101])),
      new Uint8Array([100, 101, 100, 101, 100, 101, 100]),
    );
  },
});

// tests from:
// https://github.com/nodejs/node/blob/56dbe466fdbc598baea3bfce289bf52b97b8b8f7/test/parallel/test-buffer-bytelength.js#L70
Deno.test({
  name: "Byte length is the expected for strings",
  fn() {
    // Special case: zero length string
    assertEquals(Buffer.byteLength("", "ascii"), 0);
    assertEquals(Buffer.byteLength("", "HeX"), 0);

    // utf8
    assertEquals(Buffer.byteLength("∑éllö wørl∂!", "utf-8"), 19);
    assertEquals(Buffer.byteLength("κλμνξο", "utf8"), 12);
    assertEquals(Buffer.byteLength("挵挶挷挸挹", "utf-8"), 15);
    assertEquals(Buffer.byteLength("𠝹𠱓𠱸", "UTF8"), 12);
    // Without an encoding, utf8 should be assumed
    assertEquals(Buffer.byteLength("hey there"), 9);
    assertEquals(Buffer.byteLength("𠱸挶νξ#xx :)"), 17);
    assertEquals(Buffer.byteLength("hello world", ""), 11);
    // It should also be assumed with unrecognized encoding
    assertEquals(Buffer.byteLength("hello world", "abc"), 11);
    assertEquals(Buffer.byteLength("ßœ∑≈", "unkn0wn enc0ding"), 10);

    // base64
    assertEquals(Buffer.byteLength("aGVsbG8gd29ybGQ=", "base64"), 11);
    assertEquals(Buffer.byteLength("aGVsbG8gd29ybGQ=", "BASE64"), 11);
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
        assertEquals(Buffer.byteLength("Il était tué", encoding), 12);
      });

    ["ucs2", "ucs-2", "utf16le", "utf-16le"]
      .reduce((es: string[], e: string) => es.concat(e, e.toUpperCase()), [])
      .forEach((encoding: string) => {
        assertEquals(Buffer.byteLength("Il était tué", encoding), 24);
      });
  },
});

Deno.test({
  name: "Byte length is the expected one for non-strings",
  fn() {
    assertEquals(
      Buffer.byteLength(Buffer.alloc(0)),
      Buffer.alloc(0).byteLength,
      "Byte lenght differs on buffers",
    );
  },
});

Deno.test({
  name: "Two Buffers are concatenated",
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
  name: "A single buffer concatenates and return the same buffer",
  fn() {
    const buffer1 = Buffer.alloc(1);
    const resultBuffer = Buffer.concat([buffer1]);
    assertEquals(resultBuffer.length, 1, "Buffer length should be 1");
  },
});

Deno.test({
  name: "No buffers concat returns an empty buffer",
  fn() {
    const resultBuffer = Buffer.concat([]);
    assertEquals(resultBuffer.length, 0, "Buffer length should be 0");
  },
});

Deno.test({
  name: "Buffer concat respects totalLenght parameter",
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
  name: "Buffer copy works as expected",
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
      data1,
      buffer1,
    );
  },
});

Deno.test({
  name: "Buffer copy respects the starting point for copy",
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
  name: "Buffer copy doesn't throw on offset but copies until offset reached",
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
  name: "Buffer from string creates a Buffer",
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
  name: "Buffer from string hex",
  fn() {
    for (const encoding of ["hex", "HEX"]) {
      const buffer: Buffer = Buffer.from(
        "7468697320697320612074c3a97374",
        encoding,
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
  name: "Buffer from string base64",
  fn() {
    for (const encoding of ["base64", "BASE64"]) {
      const buffer: Buffer = Buffer.from("dGhpcyBpcyBhIHTDqXN0", encoding);
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
  name: "Buffer to string base64",
  fn() {
    for (const encoding of ["base64", "BASE64"]) {
      const buffer: Buffer = Buffer.from("deno land");
      assertEquals(
        buffer.toString(encoding),
        "ZGVubyBsYW5k",
        "Buffer to string should recover the string in base64",
      );
    }
    const b64 = "dGhpcyBpcyBhIHTDqXN0";
    assertEquals(Buffer.from(b64, "base64").toString("base64"), b64);
  },
});

Deno.test({
  name: "Buffer to string hex",
  fn() {
    for (const encoding of ["hex", "HEX"]) {
      const buffer: Buffer = Buffer.from("deno land");
      assertEquals(
        buffer.toString(encoding),
        "64656e6f206c616e64",
        "Buffer to string should recover the string",
      );
    }
    const hex = "64656e6f206c616e64";
    assertEquals(Buffer.from(hex, "hex").toString("hex"), hex);
  },
});

Deno.test({
  name: "Buffer to string invalid encoding",
  fn() {
    const buffer: Buffer = Buffer.from("deno land");
    const invalidEncodings = [null, 5, {}, true, false, "foo", ""];

    for (const encoding of invalidEncodings) {
      assertThrows(
        () => {
          // deno-lint-ignore ban-ts-comment
          // @ts-expect-error
          buffer.toString(encoding);
        },
        TypeError,
        `Unkown encoding: ${encoding}`,
        "Should throw on invalid encoding",
      );
    }
  },
});

Deno.test({
  name: "Buffer from string invalid encoding",
  fn() {
    const defaultToUtf8Encodings = [null, 5, {}, true, false, ""];
    const invalidEncodings = ["deno", "base645"];

    for (const encoding of defaultToUtf8Encodings) {
      // deno-lint-ignore ban-ts-comment
      // @ts-expect-error
      assertEquals(Buffer.from("yes", encoding).toString(), "yes");
    }

    for (const encoding of invalidEncodings) {
      assertThrows(
        () => {
          Buffer.from("yes", encoding);
        },
        TypeError,
        `Unkown encoding: ${encoding}`,
      );
    }
  },
});

Deno.test({
  name: "Buffer to/from string not implemented encodings",
  fn() {
    const buffer: Buffer = Buffer.from("deno land");
    const notImplemented = ["ascii", "binary"];

    for (const encoding of notImplemented) {
      assertThrows(
        () => {
          buffer.toString(encoding);
        },
        Error,
        `"${encoding}" encoding`,
        "Should throw on invalid encoding",
      );

      assertThrows(
        () => {
          Buffer.from("", encoding);
        },
        Error,
        `"${encoding}" encoding`,
        "Should throw on invalid encoding",
      );
    }
  },
});

Deno.test({
  name: "Buffer from another buffer creates a Buffer",
  fn() {
    const buffer: Buffer = Buffer.from(Buffer.from("test"));
    assertEquals(buffer.length, 4, "Buffer length should be 4");
    assertEquals(
      buffer.toString(),
      "test",
      "Buffer to string should recover the string",
    );
  },
});

Deno.test({
  name: "isBuffer returns true if the object is a buffer",
  fn() {
    assertEquals(Buffer.isBuffer(Buffer.from("test")), true);
  },
});

Deno.test({
  name: "isBuffer returns false if the object is not a buffer",
  fn() {
    assertEquals(Buffer.isBuffer({ test: 3 }), false);
    assertEquals(Buffer.isBuffer(new Uint8Array()), false);
  },
});

Deno.test({
  name: "Buffer toJSON",
  fn() {
    assertEquals(
      JSON.stringify(Buffer.from("deno")),
      '{"type":"Buffer","data":[100,101,110,111]}',
    );
  },
});

Deno.test({
  name: "buf.slice does not create a copy",
  fn() {
    const buf = Buffer.from("ceno");
    // This method is not compatible with the Uint8Array.prototype.slice()
    const slice = buf.slice();
    slice[0]++;
    assertEquals(slice.toString(), "deno");
  },
});

Deno.test({
  name: "isEncoding returns true for valid encodings",
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
  name: "isEncoding returns false for invalid encodings",
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
      assertEquals(Buffer.isEncoding(enc), false);
    });
  },
});

// ported from:
// https://github.com/nodejs/node/blob/56dbe466fdbc598baea3bfce289bf52b97b8b8f7/test/parallel/test-buffer-equals.js#L6
Deno.test({
  name: "buf.equals",
  fn() {
    const b = Buffer.from("abcdf");
    const c = Buffer.from("abcdf");
    const d = Buffer.from("abcde");
    const e = Buffer.from("abcdef");

    assertEquals(b.equals(c), true);
    assertEquals(d.equals(d), true);
    assertEquals(
      d.equals(new Uint8Array([0x61, 0x62, 0x63, 0x64, 0x65])),
      true,
    );

    assertEquals(c.equals(d), false);
    assertEquals(d.equals(e), false);

    assertThrows(
      // deno-lint-ignore ban-ts-comment
      // @ts-expect-error
      () => Buffer.alloc(1).equals("abc"),
      TypeError,
      `The "otherBuffer" argument must be an instance of Buffer or Uint8Array. Received type string`,
    );
  },
});
