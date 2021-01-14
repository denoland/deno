// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/** Create a `Writer` from a `WritableStreamDefaultReader`. */
export function writerFromStreamWriter(
  streamWriter: WritableStreamDefaultWriter<Uint8Array>,
): Deno.Writer {
  return {
    async write(p: Uint8Array): Promise<number> {
      await streamWriter.ready;
      await streamWriter.write(p);
      return p.length;
    },
  };
}

/** Create a `Reader` from a `ReadableStreamDefaultReader`. */
export function readerFromStreamReader(
  streamReader: ReadableStreamDefaultReader<Uint8Array>,
): Deno.Reader {
  const buffer = new Deno.Buffer();

  return {
    async read(p: Uint8Array): Promise<number | null> {
      if (buffer.empty()) {
        const res = await streamReader.read();
        if (res.done) {
          return null; // EOF
        }

        await Deno.writeAll(buffer, res.value);
      }

      return buffer.read(p);
    },
  };
}

/** Create a `WritableStream` from a `Writer`. */
export function writableStreamFromWriter(
  writer: Deno.Writer,
): WritableStream<Uint8Array> {
  return new WritableStream({
    async write(chunk) {
      await Deno.writeAll(writer, chunk);
    },
  });
}

/** Create a `ReadableStream` from an `AsyncIterator`. */
export function readableStreamFromAsyncIterator<T>(
  iterator: AsyncIterableIterator<T>,
): ReadableStream<T> {
  return new ReadableStream({
    async pull(controller) {
      const { value, done } = await iterator.next();

      if (done) {
        controller.close();
      } else {
        controller.enqueue(value);
      }
    },
  });
}
