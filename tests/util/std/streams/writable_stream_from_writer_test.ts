// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { writableStreamFromWriter } from "./writable_stream_from_writer.ts";
import type { Closer, Writer } from "../types.d.ts";

class MockWriterCloser implements Writer, Closer {
  chunks: Uint8Array[] = [];
  closeCall = 0;

  write(p: Uint8Array): Promise<number> {
    if (this.closeCall) {
      throw new Error("already closed");
    }
    if (p.length) {
      this.chunks.push(p);
    }
    return Promise.resolve(p.length);
  }

  close() {
    this.closeCall++;
  }
}

Deno.test("[streams] writableStreamFromWriter()", async function () {
  const written: string[] = [];
  const chunks: string[] = ["hello", "deno", "land"];
  const decoder = new TextDecoder();

  // deno-lint-ignore require-await
  async function write(p: Uint8Array): Promise<number> {
    written.push(decoder.decode(p));
    return p.length;
  }

  const writableStream = writableStreamFromWriter({ write });

  const encoder = new TextEncoder();
  const streamWriter = writableStream.getWriter();
  for (const chunk of chunks) {
    await streamWriter.write(encoder.encode(chunk));
  }

  assertEquals(written, chunks);
});

Deno.test("[streams] writableStreamFromWriter() - calls close on close", async function () {
  const written: string[] = [];
  const chunks: string[] = ["hello", "deno", "land"];
  const decoder = new TextDecoder();

  const writer = new MockWriterCloser();
  const writableStream = writableStreamFromWriter(writer);

  const encoder = new TextEncoder();
  const streamWriter = writableStream.getWriter();
  for (const chunk of chunks) {
    await streamWriter.write(encoder.encode(chunk));
  }
  await streamWriter.close();

  for (const chunk of writer.chunks) {
    written.push(decoder.decode(chunk));
  }

  assertEquals(written, chunks);
  assertEquals(writer.closeCall, 1);
});

Deno.test("[streams] writableStreamFromWriter() - calls close on abort", async function () {
  const written: string[] = [];
  const chunks: string[] = ["hello", "deno", "land"];
  const decoder = new TextDecoder();

  const writer = new MockWriterCloser();
  const writableStream = writableStreamFromWriter(writer);

  const encoder = new TextEncoder();
  const streamWriter = writableStream.getWriter();
  for (const chunk of chunks) {
    await streamWriter.write(encoder.encode(chunk));
  }
  await streamWriter.abort();

  for (const chunk of writer.chunks) {
    written.push(decoder.decode(chunk));
  }

  assertEquals(written, chunks);
  assertEquals(writer.closeCall, 1);
});

Deno.test("[streams] writableStreamFromWriter() - doesn't call close with autoClose false", async function () {
  const written: string[] = [];
  const chunks: string[] = ["hello", "deno", "land"];
  const decoder = new TextDecoder();

  const writer = new MockWriterCloser();
  const writableStream = writableStreamFromWriter(writer, { autoClose: false });

  const encoder = new TextEncoder();
  const streamWriter = writableStream.getWriter();
  for (const chunk of chunks) {
    await streamWriter.write(encoder.encode(chunk));
  }
  await streamWriter.close();

  for (const chunk of writer.chunks) {
    written.push(decoder.decode(chunk));
  }

  assertEquals(written, chunks);
  assertEquals(writer.closeCall, 0);
});
