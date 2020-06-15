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
