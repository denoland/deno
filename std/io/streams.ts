// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** Creates a Writer from a WritableStreamDefaultWriter */
export function fromStreamWriter(
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
/** Creates a Reader from a ReadableStreamDefaultReader */
export function fromStreamReader(
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
