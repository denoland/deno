import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import Buffer from "./buffer.ts";

Deno.test({
  name: "alloc fails on negative numbers",
  fn() {
    assertThrows(
      () => {
        Buffer.alloc(-1);
      },
      RangeError,
      "Invalid typed array length: -1",
      "should throw on negative numbers"
    );
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
  name: "Byte length is the expected for strings",
  fn() {
    assertEquals(Buffer.byteLength("test"), 4, "Byte lenght differs on string");
  },
});

Deno.test({
  name: "Byte length is the expected one for non-strings",
  fn() {
    assertEquals(
      Buffer.byteLength(Buffer.alloc(0)),
      Buffer.alloc(0).byteLength,
      "Byte lenght differs on buffers"
    );
  },
});

Deno.test({
  name: "Two Buffers are concatenated",
  fn() {
    const buffer1 = Buffer.alloc(1);
    const buffer2 = Buffer.alloc(2);
    const resultBuffer = Buffer.concat([buffer1, buffer2]);
    assertEquals(resultBuffer.length, 3, "Buffer length should be 3");
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
  name: "concat respects totalLenght parameter",
  fn() {
    const buffer1 = Buffer.alloc(2);
    const buffer2 = Buffer.alloc(2);
    const resultBuffer = Buffer.concat([buffer1, buffer2], 10);
    assertEquals(resultBuffer.length, 10, "Buffer length should be 10");
  },
});

Deno.test({
  name: "concat totalLenght throws if is lower than the size of the buffers",
  fn() {
    const buffer1 = Buffer.alloc(2);
    const buffer2 = Buffer.alloc(2);
    assertThrows(
      () => {
        Buffer.concat([buffer1, buffer2], 3);
      },
      RangeError,
      "offset is out of bounds",
      "should throw on negative numbers"
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
      "Buffer to string should recover the string"
    );
  },
});

Deno.test({
  name: "Buffer from string hex",
  fn() {
    for (const encoding of ["hex", "HEX"]) {
      const buffer: Buffer = Buffer.from(
        "7468697320697320612074c3a97374",
        encoding
      );
      assertEquals(buffer.length, 15, "Buffer length should be 15");
      assertEquals(
        buffer.toString(),
        "this is a tést",
        "Buffer to string should recover the string"
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
        "Buffer to string should recover the string"
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
        "Buffer to string should recover the string in base64"
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
        "Buffer to string should recover the string"
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
          // @ts-ignore
          buffer.toString(encoding);
        },
        TypeError,
        `Unkown encoding: ${encoding}`,
        "Should throw on invalid encoding"
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
      // @ts-ignore
      assertEquals(Buffer.from("yes", encoding).toString(), "yes");
    }

    for (const encoding of invalidEncodings) {
      assertThrows(
        () => {
          // deno-lint-ignore ban-ts-comment
          // @ts-ignore
          Buffer.from("yes", encoding);
        },
        TypeError,
        `Unkown encoding: ${encoding}`
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
          // deno-lint-ignore ban-ts-comment
          // @ts-ignore
          buffer.toString(encoding);
        },
        Error,
        `"${encoding}" encoding`,
        "Should throw on invalid encoding"
      );

      assertThrows(
        () => {
          // deno-lint-ignore ban-ts-comment
          // @ts-ignore
          Buffer.from("", encoding);
        },
        Error,
        `"${encoding}" encoding`,
        "Should throw on invalid encoding"
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
      "Buffer to string should recover the string"
    );
  },
});

Deno.test({
  name: "isBuffer returns true if the object is a buffer",
  fn() {
    assert(Buffer.isBuffer(Buffer.from("test")));
  },
});

Deno.test({
  name: "isBuffer returns false if the object is not a buffer",
  fn() {
    assert(!Buffer.isBuffer({ test: 3 }));
  },
});
