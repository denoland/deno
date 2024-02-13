// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { StringWriter } from "./string_writer.ts";
import { StringReader } from "./string_reader.ts";
import { copyN } from "./copy_n.ts";
import { copy } from "../streams/copy.ts";

Deno.test("ioStringWriter", async function () {
  const w = new StringWriter("base");
  const r = new StringReader("0123456789");
  await copyN(r, w, 4);
  assertEquals(w.toString(), "base0123");
  await copy(r, w);
  assertEquals(w.toString(), "base0123456789");
});

Deno.test("ioStringWriterSync", function () {
  const encoder = new TextEncoder();
  const w = new StringWriter("");
  w.writeSync(encoder.encode("deno"));
  assertEquals(w.toString(), "deno");
  w.writeSync(encoder.encode("\nland"));
  assertEquals(w.toString(), "deno\nland");
});

Deno.test("ioStringWriterIsolationTest", async function () {
  const encoder = new TextEncoder();
  const src = "ABC";
  const srcChunks = src.split("").map((c) => encoder.encode(c));

  const w = new StringWriter();
  for (const c of srcChunks) {
    const written = await w.write(c);
    assertEquals(written, 1);
  }
  srcChunks[0][0] = 88;
  assertEquals(w.toString(), src);
});
