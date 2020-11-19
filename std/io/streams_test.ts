// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../testing/asserts.ts";
import {
  readableStreamFromAsyncIterator,
  readerFromStreamReader,
  writableStreamFromWriter,
  writerFromStreamWriter,
} from "./streams.ts";

function repeat(c: string, bytes: number): Uint8Array {
  assertEquals(c.length, 1);
  const ui8 = new Uint8Array(bytes);
  ui8.fill(c.charCodeAt(0));
  return ui8;
}

Deno.test("toWriterCheck", async function (): Promise<void> {
  const written: string[] = [];
  const chunks: string[] = ["hello", "deno", "land"];
  const writableStream = new WritableStream({
    write(chunk): void {
      const decoder = new TextDecoder();
      written.push(decoder.decode(chunk));
    },
  });

  const encoder = new TextEncoder();
  const writer = writerFromStreamWriter(writableStream.getWriter());

  for (const chunk of chunks) {
    const n = await writer.write(encoder.encode(chunk));
    // stream writers always write all the bytes
    assertEquals(n, chunk.length);
  }

  assertEquals(written, chunks);
});

Deno.test("toReaderCheck", async function (): Promise<void> {
  const chunks: string[] = ["hello", "deno", "land"];
  const expected = chunks.slice();
  const readChunks: Uint8Array[] = [];
  const readableStream = new ReadableStream({
    pull(controller): void {
      const encoder = new TextEncoder();
      const chunk = chunks.shift();
      if (!chunk) return controller.close();
      controller.enqueue(encoder.encode(chunk));
    },
  });

  const decoder = new TextDecoder();
  const reader = readerFromStreamReader(readableStream.getReader());

  let i = 0;

  while (true) {
    const b = new Uint8Array(1024);
    const n = await reader.read(b);

    if (n === null) break;

    readChunks.push(b.subarray(0, n));
    assert(i < expected.length);

    i++;
  }

  assertEquals(
    expected,
    readChunks.map((chunk) => decoder.decode(chunk)),
  );
});

Deno.test("toReaderBigChunksCheck", async function (): Promise<void> {
  const bufSize = 1024;
  const chunkSize = 3 * bufSize;
  const writer = new Deno.Buffer();

  // A readable stream can enqueue chunks bigger than Copy bufSize
  // Reader returned by toReader should enqueue exceeding bytes
  const chunks: string[] = [
    "a".repeat(chunkSize),
    "b".repeat(chunkSize),
    "c".repeat(chunkSize),
  ];
  const expected = chunks.slice();
  const readableStream = new ReadableStream({
    pull(controller): void {
      const encoder = new TextEncoder();
      const chunk = chunks.shift();
      if (!chunk) return controller.close();

      controller.enqueue(encoder.encode(chunk));
    },
  });

  const reader = readerFromStreamReader(readableStream.getReader());
  const n = await Deno.copy(reader, writer, { bufSize });

  const expectedWritten = chunkSize * expected.length;
  assertEquals(n, chunkSize * expected.length);
  assertEquals(writer.length, expectedWritten);
});

Deno.test("toReaderBigIrregularChunksCheck", async function (): Promise<void> {
  const bufSize = 1024;
  const chunkSize = 3 * bufSize;
  const writer = new Deno.Buffer();

  // A readable stream can enqueue chunks bigger than Copy bufSize
  // Reader returned by toReader should enqueue exceeding bytes
  const chunks: Uint8Array[] = [
    repeat("a", chunkSize),
    repeat("b", chunkSize + 253),
    repeat("c", chunkSize + 8),
  ];
  const expected = new Uint8Array(
    chunks
      .slice()
      .map((chunk) => [...chunk])
      .flat(),
  );
  const readableStream = new ReadableStream({
    pull(controller): void {
      const chunk = chunks.shift();
      if (!chunk) return controller.close();

      controller.enqueue(chunk);
    },
  });

  const reader = readerFromStreamReader(readableStream.getReader());

  const n = await Deno.copy(reader, writer, { bufSize });
  assertEquals(n, expected.length);
  assertEquals(expected, writer.bytes());
});

Deno.test("toWritableCheck", async function (): Promise<void> {
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

Deno.test("toReadableCheck", async function (): Promise<void> {
  const chunks: string[] = ["hello", "deno", "land"];
  const expected = chunks.slice();
  const readChunks: string[] = [];
  const encoder = new TextEncoder();

  // deno-lint-ignore require-await
  async function read(p: Uint8Array): Promise<number | null> {
    const chunk = chunks.shift();
    if (chunk === undefined) {
      return null;
    } else {
      const encoded = encoder.encode(chunk);
      p.set(encoded);
      return encoded.length;
    }
  }
  const iter = Deno.iter({ read });
  const writableStream = readableStreamFromAsyncIterator(iter);

  const decoder = new TextDecoder();
  for await (const chunk of writableStream.getIterator()) {
    readChunks.push(decoder.decode(chunk));
  }

  assertEquals(expected, readChunks);
});
