// Copyright 2018-2026 the Deno authors. MIT license.
import { Buffer } from "node:buffer";
import { deepStrictEqual } from "node:assert";

// deno bench tests/bench/buffer_utf16le.js
// Run unchanged on baseline and candidate binaries. Both implementations return
// newly allocated bytes; the write cases isolate conversion from allocation.
let sink = 0;
for (
  const [corpus, phrase] of [
    ["ascii", "Hello world! "],
    ["latin1", "café déjà vu "],
    ["utf16", "Hello λ 😀 漢字! "],
  ]
) {
  for (const size of [32, 2048, 32768]) {
    const text = phrase.repeat(Math.ceil(size / phrase.length)).slice(0, size);
    const manual = () => {
      const bytes = new Uint8Array(text.length * 2);
      const view = new DataView(bytes.buffer);
      for (let i = 0; i < text.length; i++) {
        view.setUint16(i * 2, text.charCodeAt(i), true);
      }
      return bytes;
    };
    const expected = manual();
    deepStrictEqual(new Uint8Array(Buffer.from(text, "utf16le")), expected);
    const group = `${corpus}/${size * 2} bytes`;
    Deno.bench({
      name: `DataView ${group}`,
      group,
      baseline: true,
      fn() {
        const result = manual();
        sink ^= result[sink % result.length];
      },
    });
    Deno.bench({
      name: `Buffer.from ${group}`,
      group,
      fn() {
        const result = Buffer.from(text, "utf16le");
        sink ^= result[sink % result.length];
      },
    });
    for (const offset of [0, 1]) {
      const target = Buffer.alloc(text.length * 2 + offset);
      deepStrictEqual(
        target.subarray(offset, offset + target.write(text, offset, "utf16le")),
        Buffer.from(expected),
      );
      Deno.bench({
        name: `Buffer.write offset=${offset} ${group}`,
        group,
        fn() {
          sink ^= target.write(text, offset, "utf16le");
        },
      });
    }
  }
}
