// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { readableStreamFromReader } from "./readable_stream_from_reader.ts";
import { Buffer } from "../io/buffer.ts";
import { concat } from "../bytes/concat.ts";
import { copy } from "../bytes/copy.ts";
import type { Closer, Reader } from "../types.d.ts";

class MockReaderCloser implements Reader, Closer {
  chunks: Uint8Array[] = [];
  closeCall = 0;

  read(p: Uint8Array): Promise<number | null> {
    if (this.closeCall) {
      throw new Error("already closed");
    }
    if (p.length === 0) {
      return Promise.resolve(0);
    }
    const chunk = this.chunks.shift();
    if (chunk) {
      const copied = copy(chunk, p);
      if (copied < chunk.length) {
        this.chunks.unshift(chunk.subarray(copied));
      }
      return Promise.resolve(copied);
    }
    return Promise.resolve(null);
  }

  close() {
    this.closeCall++;
  }
}

Deno.test("[streams] readableStreamFromReader()", async function () {
  const encoder = new TextEncoder();
  const reader = new Buffer(encoder.encode("hello deno land"));
  const stream = readableStreamFromReader(reader);
  const actual = await Array.fromAsync(stream);
  const decoder = new TextDecoder();
  assertEquals(decoder.decode(concat(actual)), "hello deno land");
});

Deno.test({
  name: "[streams] readableStreamFromReader() auto closes closer",
  async fn() {},
});

Deno.test("[streams] readableStreamFromReader() - calls close", async function () {
  const encoder = new TextEncoder();
  const reader = new MockReaderCloser();
  reader.chunks = [
    encoder.encode("hello "),
    encoder.encode("deno "),
    encoder.encode("land"),
  ];
  const stream = readableStreamFromReader(reader);
  const actual = await Array.fromAsync(stream);
  const decoder = new TextDecoder();
  assertEquals(decoder.decode(concat(actual)), "hello deno land");
  assertEquals(reader.closeCall, 1);
});

Deno.test("[streams] readableStreamFromReader() - doesn't call close with autoClose false", async function () {
  const encoder = new TextEncoder();
  const reader = new MockReaderCloser();
  reader.chunks = [
    encoder.encode("hello "),
    encoder.encode("deno "),
    encoder.encode("land"),
  ];
  const stream = readableStreamFromReader(reader, { autoClose: false });
  const actual = await Array.fromAsync(stream);
  const decoder = new TextDecoder();
  assertEquals(decoder.decode(concat(actual)), "hello deno land");
  assertEquals(reader.closeCall, 0);
});

Deno.test("[streams] readableStreamFromReader() - chunkSize", async function () {
  const encoder = new TextEncoder();
  const reader = new MockReaderCloser();
  reader.chunks = [
    encoder.encode("hello "),
    encoder.encode("deno "),
    encoder.encode("land"),
  ];
  const stream = readableStreamFromReader(reader, { chunkSize: 2 });
  const actual = await Array.fromAsync(stream);
  const decoder = new TextDecoder();
  assertEquals(actual.length, 8);
  assertEquals(decoder.decode(concat(actual)), "hello deno land");
  assertEquals(reader.closeCall, 1);
});
