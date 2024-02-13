// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This code has been ported almost directly from Go's src/bytes/buffer_test.go
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
import { assertEquals } from "../assert/mod.ts";
import { BufWriter, BufWriterSync } from "./buf_writer.ts";
import { Buffer } from "./buffer.ts";
import { StringWriter } from "./string_writer.ts";
import { bufsizes } from "./_test_common.ts";
import type { Writer, WriterSync } from "../types.d.ts";

Deno.test("bufioWriter", async function () {
  const data = new Uint8Array(8192);

  for (let i = 0; i < data.byteLength; i++) {
    data[i] = " ".charCodeAt(0) + (i % ("~".charCodeAt(0) - " ".charCodeAt(0)));
  }

  const w = new Buffer();
  for (const nwrite of bufsizes) {
    for (const bs of bufsizes) {
      // Write nwrite bytes using buffer size bs.
      // Check that the right amount makes it out
      // and that the data is correct.

      w.reset();
      const buf = new BufWriter(w, bs);

      const context = `nwrite=${nwrite} bufsize=${bs}`;
      const n = await buf.write(data.subarray(0, nwrite));
      assertEquals(n, nwrite, context);

      await buf.flush();

      const written = w.bytes();
      assertEquals(written.byteLength, nwrite);

      for (let l = 0; l < written.byteLength; l++) {
        assertEquals(written[l], data[l]);
      }
    }
  }
});

Deno.test("bufioWriterSync", function () {
  const data = new Uint8Array(8192);

  for (let i = 0; i < data.byteLength; i++) {
    data[i] = " ".charCodeAt(0) + (i % ("~".charCodeAt(0) - " ".charCodeAt(0)));
  }

  const w = new Buffer();
  for (const nwrite of bufsizes) {
    for (const bs of bufsizes) {
      // Write nwrite bytes using buffer size bs.
      // Check that the right amount makes it out
      // and that the data is correct.

      w.reset();
      const buf = new BufWriterSync(w, bs);

      const context = `nwrite=${nwrite} bufsize=${bs}`;
      const n = buf.writeSync(data.subarray(0, nwrite));
      assertEquals(n, nwrite, context);

      buf.flush();

      const written = w.bytes();
      assertEquals(written.byteLength, nwrite);

      for (let l = 0; l < written.byteLength; l++) {
        assertEquals(written[l], data[l]);
      }
    }
  }
});

Deno.test({
  name: "Reset buffer after flush",
  async fn() {
    const stringWriter = new StringWriter();
    const bufWriter = new BufWriter(stringWriter);
    const encoder = new TextEncoder();
    await bufWriter.write(encoder.encode("hello\nworld\nhow\nare\nyou?\n\n"));
    await bufWriter.flush();
    await bufWriter.write(encoder.encode("foobar\n\n"));
    await bufWriter.flush();
    const actual = stringWriter.toString();
    assertEquals(actual, "hello\nworld\nhow\nare\nyou?\n\nfoobar\n\n");
  },
});

Deno.test({
  name: "Reset buffer after flush sync",
  fn() {
    const stringWriter = new StringWriter();
    const bufWriter = new BufWriterSync(stringWriter);
    const encoder = new TextEncoder();
    bufWriter.writeSync(encoder.encode("hello\nworld\nhow\nare\nyou?\n\n"));
    bufWriter.flush();
    bufWriter.writeSync(encoder.encode("foobar\n\n"));
    bufWriter.flush();
    const actual = stringWriter.toString();
    assertEquals(actual, "hello\nworld\nhow\nare\nyou?\n\nfoobar\n\n");
  },
});

Deno.test({
  name: "BufWriter.flush should write all bytes",
  async fn() {
    const bufSize = 16 * 1024;
    const data = new Uint8Array(bufSize);
    data.fill("a".charCodeAt(0));

    const cache: Uint8Array[] = [];
    const writer: Writer = {
      write(p: Uint8Array): Promise<number> {
        cache.push(p.subarray(0, 1));

        // Writer that only writes 1 byte at a time
        return Promise.resolve(1);
      },
    };

    const bufWriter = new BufWriter(writer);
    await bufWriter.write(data);

    await bufWriter.flush();
    const buf = new Uint8Array(cache.length);
    for (let i = 0; i < cache.length; i++) buf.set(cache[i], i);

    assertEquals(data, buf);
  },
});

Deno.test({
  name: "BufWriterSync.flush should write all bytes",
  fn() {
    const bufSize = 16 * 1024;
    const data = new Uint8Array(bufSize);
    data.fill("a".charCodeAt(0));

    const cache: Uint8Array[] = [];
    const writer: WriterSync = {
      writeSync(p: Uint8Array): number {
        cache.push(p.subarray(0, 1));
        // Writer that only writes 1 byte at a time
        return 1;
      },
    };

    const bufWriter = new BufWriterSync(writer);
    bufWriter.writeSync(data);

    bufWriter.flush();
    const buf = new Uint8Array(cache.length);
    for (let i = 0; i < cache.length; i++) buf.set(cache[i], i);

    assertEquals(data, buf);
  },
});
